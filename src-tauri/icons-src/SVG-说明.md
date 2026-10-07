# 艾森豪威尔四象限任务清单 - App 图标 SVG 包

## 文件清单

| 文件 | 用途 | 说明 |
|---|---|---|
| core-icon.svg | 通用透明底图标 | 图标居中，周围透明，可用于网页、文档、任何场景 |
| ios-macos.svg | iOS / macOS | 全屏出血版本，系统自动加圆角蒙版，导出 1024×1024 PNG 后上传 App Store |
| android-foreground.svg | Android 自适应图标前景层 | 居中缩小版，适配自适应图标安全区 |
| android-background.svg | Android 自适应图标背景层 | 淡粉白→淡蓝渐变底，与前景层叠加 |
| windows.svg | Windows | 小圆角方形，适合开始菜单/任务栏/桌面 |

## 配色

- 左上：#FF9A8B → #FFB88C（粉橙渐变）
- 右上：#B8D4FF → #5EB8FF（天蓝渐变）
- 左下：#C8F0DC → #5EE8B0（薄荷绿渐变）
- 右下：#E0CCFF → #A890FF（淡紫渐变）

## 使用提示

1. SVG 为矢量格式，可无损缩放至任意尺寸
2. 导出 PNG 时建议尺寸：iOS 1024×1024，Android 108dp（216px 以上），Windows 256/48/16px
3. Android 使用时将 foreground + background 两层在 Android Studio 中组合为 Adaptive Icon
4. 如需调整颜色，直接修改 SVG 中 stop 标签的 stop-color 值即可
