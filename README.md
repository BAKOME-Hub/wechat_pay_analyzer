# 💰 BAKOME WeChat Pay Analyzer – 微信支付账单分析器

**高性能 Rust 工具，解析微信支付 CSV 账单，生成消费分类、月度趋势、商家排名及交互式 HTML 报告。隐私优先，本地处理。**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange)](https://www.rust-lang.org/)
[![WeChat Pay](https://img.shields.io/badge/WeChat-Pay-07C160)](https://pay.weixin.qq.com/)

---

## ✨ 功能特性

| 功能 | 说明 |
|------|------|
| **CSV 解析** | 直接读取微信支付导出的账单文件 |
| **自动分类** | 根据商户名/商品描述智能分类（餐饮、交通、购物、红包等） |
| **统计报表** | 总收入、总支出、净储蓄、日均支出、交易笔数 |
| **可视化图表** | 消费分类饼图、月度收支趋势图（Chart.js） |
| **商家排行** | 消费最多的前 10 名商家 |
| **月度明细** | 逐月收支对比表 |
| **输出格式** | 交互式 HTML 报告（离线可用）+ 可选 JSON |
| **隐私安全** | 所有数据本地处理，不上传任何信息 |

---

## 🚀 快速开始

### 1. 导出微信支付账单

- 打开微信 → 我 → 服务 → 钱包 → 账单 → 常见问题 → 下载账单 → 选择「用于个人对账」→ 自定义时间 → 输入邮箱接收 CSV 文件。

### 2. 安装 Rust 环境

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
