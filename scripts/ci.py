#!/usr/bin/env python3
"""在命令行查看本仓库 GitHub Actions 的运行状态与失败日志。

为什么需要它：这个项目按约定「不在本机打包」，一切构建都在 GitHub 上跑，
而本机没有 gh CLI、git 走 schannel 时连不通 GitHub。这个脚本只用标准库 +
git 已保存的凭据，通过 HTTP 代理访问公开 API，因此可以直接远程看构建结果。

用法（在仓库根目录执行，python 用系统/自带均可）：

  python scripts/ci.py runs   [分支] [工作流文件]     列出最近几次运行
  python scripts/ci.py watch  [分支] [工作流文件]     轮询到最近一次运行结束，失败时打印失败步骤日志
  python scripts/ci.py steps  <run_id>               每个步骤的结论
  python scripts/ci.py artifacts <run_id>            该次运行的产物（名字/大小/是否过期）
  python scripts/ci.py fetch  <artifact_id> <输出路径>  下载产物 zip（APK 等）
  python scripts/ci.py log    <job_id> [正则]         打印 job 日志；给了正则会过滤匹配行

环境变量：
  HTTPS_PROXY / HTTP_PROXY   代理地址，默认 http://127.0.0.1:7890
  GITHUB_TOKEN               可选；不设时用 `git credential fill` 取已保存的凭据
  GITHUB_REPO                可选；默认从 `git remote get-url origin` 推断
"""

import http.client
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

DEFAULT_PROXY = "http://127.0.0.1:7890"
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RETRIES = 4


def repo_slug():
    if os.environ.get("GITHUB_REPO"):
        return os.environ["GITHUB_REPO"]
    url = subprocess.run(
        ["git", "-C", ROOT, "remote", "get-url", "origin"],
        capture_output=True, text=True, check=True).stdout.strip()
    match = re.search(r"github\.com[:/](.+?)(?:\.git)?$", url)
    if not match:
        raise SystemExit(f"无法从 remote 推断仓库: {url}")
    return match.group(1)


API = "https://api.github.com/repos/" + repo_slug()
PROXY = os.environ.get("HTTPS_PROXY") or os.environ.get("HTTP_PROXY") or DEFAULT_PROXY
if PROXY:
    os.environ.setdefault("HTTPS_PROXY", PROXY)
    os.environ.setdefault("HTTP_PROXY", PROXY)


def token():
    if os.environ.get("GITHUB_TOKEN"):
        return os.environ["GITHUB_TOKEN"]
    out = subprocess.run(
        ["git", "-C", ROOT, "credential", "fill"],
        input="protocol=https\nhost=github.com\n\n",
        capture_output=True, text=True,
        env={**os.environ, "GIT_TERMINAL_PROMPT": "0"})
    for line in out.stdout.splitlines():
        if line.startswith("password="):
            return line[len("password="):]
    raise SystemExit("没有可用的 GitHub 凭据：请设置 GITHUB_TOKEN 或先 git push 一次")


class _NoAuthRedirect(urllib.request.HTTPRedirectHandler):
    """产物下载会跳到 Azure blob，那里不接受 Bearer token，跳转时必须去掉。"""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        new = super().redirect_request(req, fp, code, msg, headers, newurl)
        if new is not None:
            new.headers.pop("Authorization", None)
        return new


def _request(path, raw):
    req = urllib.request.Request(
        API + path,
        headers={"Authorization": "Bearer " + token(), "User-Agent": "dsh",
                 "Accept": "application/vnd.github+json"})
    opener = urllib.request.build_opener(
        urllib.request.ProxyHandler({"http": PROXY, "https": PROXY}), _NoAuthRedirect())
    # 代理偶尔会掐断连接或截断响应（IncompleteRead / 半截 JSON），重试比让整轮观察中断划算。
    last = None
    for attempt in range(RETRIES):
        try:
            data = opener.open(req, timeout=300).read()
            return data if raw else json.loads(data.decode("utf-8", "replace"))
        except (urllib.error.URLError, http.client.HTTPException, TimeoutError,
                ConnectionError, OSError, ValueError) as err:
            last = err
            print(f"  (网络重试 {attempt + 1}/{RETRIES}: {type(err).__name__})", flush=True)
            time.sleep(3 * (attempt + 1))
    raise SystemExit(f"访问 GitHub API 失败: {last}")


def get(path):
    return _request(path, raw=False)


def get_bytes(path):
    return _request(path, raw=True)


def newest_run(branch, workflow):
    data = get(f"/actions/workflows/{workflow}/runs?per_page=5&branch={branch}")
    runs = data.get("workflow_runs", [])
    return runs[0] if runs else None


def print_steps(run_id):
    jobs = get(f"/actions/runs/{run_id}/jobs?per_page=20")["jobs"]
    for job in jobs:
        print(f"JOB {job['name']}: {job['status']}/{job['conclusion']}")
        for step in job["steps"]:
            mark = "" if step["conclusion"] == "success" else "  <<<"
            print(f"   {step['name'][:52]:<54} {step['conclusion']}{mark}")
    return jobs


NOISE = re.compile(
    r"License Agreement|Accept\? \(y/N\)|^\s*\d+\.\d+|^----|^Terms and Conditions|"
    r"^This is the|^You and Google|^August|^June|^November|^\s*$")


def print_failure(job_id, pattern=None, before=26, after=16):
    text = get_bytes(f"/actions/jobs/{job_id}/logs").decode("utf-8", "replace")
    lines = [re.sub(r"^\S+Z ", "", line) for line in text.splitlines()]
    lines = [line for line in lines if len(line) < 400 and not NOISE.search(line)]
    if pattern:
        hits = [line for line in lines if re.search(pattern, line, re.I)]
        print(f"=== {len(hits)} 行匹配 {pattern!r}")
        for line in hits[:80]:
            print(line[:300])
        return
    idx = next((i for i, line in enumerate(lines)
                if re.search(r"failed with exit code|##\[error\]|error:", line, re.I)), None)
    if idx is None:
        print("=== 没有错误标记，打印最后 40 行")
        for line in lines[-40:]:
            print(line[:300])
        return
    for line in lines[max(0, idx - before): idx + after]:
        print(line[:300])


def print_artifacts(run_id):
    items = get(f"/actions/runs/{run_id}/artifacts")["artifacts"]
    if not items:
        print("（这次运行没有产物）")
    for item in items:
        size = item["size_in_bytes"] / 1024 / 1024
        print(f"{item['id']} {item['name']} {size:.1f}MB expired={item['expired']}")
    return items


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    cmd = sys.argv[1]
    if cmd == "runs":
        branch = sys.argv[2] if len(sys.argv) > 2 else "main"
        for run in get(f"/actions/runs?per_page=10&branch={branch}")["workflow_runs"]:
            print(f"{run['id']} {run['name']:<10} {run['head_branch']:<16} "
                  f"{run['status']}/{run['conclusion']} {run['html_url']}")
    elif cmd == "watch":
        branch = sys.argv[2] if len(sys.argv) > 2 else "main"
        workflow = sys.argv[3] if len(sys.argv) > 3 else "android.yml"
        run = None
        for _ in range(160):  # 约 80 分钟
            run = newest_run(branch, workflow)
            if run:
                print(f"[{time.strftime('%H:%M:%S')}] {run['id']} "
                      f"{run['status']}/{run['conclusion']}", flush=True)
                if run["status"] == "completed":
                    break
            time.sleep(30)
        if not run:
            print("没找到运行记录")
            return
        print("RUN", run["html_url"])
        jobs = print_steps(run["id"])
        print_artifacts(run["id"])
        if run["conclusion"] != "success":
            bad = next((job for job in jobs if job["conclusion"] == "failure"), None)
            if bad:
                print(f"=== 失败日志（job {bad['id']}: {bad['name']}）")
                print_failure(bad["id"])
    elif cmd == "steps":
        print_steps(sys.argv[2])
    elif cmd == "artifacts":
        print_artifacts(sys.argv[2])
    elif cmd == "fetch":
        data = get_bytes(f"/actions/artifacts/{sys.argv[2]}/zip")
        with open(sys.argv[3], "wb") as handle:
            handle.write(data)
        print(f"已保存 {len(data)} 字节到 {sys.argv[3]}")
    elif cmd == "log":
        print_failure(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else None)
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
