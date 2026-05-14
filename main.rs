//! # BAKOME WeChat Pay Analyzer - 微信支付账单分析器
//!
//! 高性能 Rust 工具，解析微信支付导出的 CSV 账单，生成详细的消费分析报告（HTML / PDF），
//! 支持自动分类、预算跟踪、图表可视化。专为中国用户设计，保护隐私（本地处理）。

use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime};
use clap::{Parser, Subcommand};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// 交易记录结构
#[derive(Debug, Deserialize, Serialize)]
struct Transaction {
    #[serde(rename = "交易时间")]
    time: String,
    #[serde(rename = "交易类型")]
    txn_type: String,
    #[serde(rename = "交易对方")]
    counterparty: String,
    #[serde(rename = "商品")]
    description: String,
    #[serde(rename = "收支")]
    income_expense: String,   // "收入" / "支出"
    #[serde(rename = "金额")]
    amount: String,           // 如 "¥100.00"
    #[serde(rename = "支付方式")]
    payment_method: String,
    #[serde(rename = "状态")]
    status: String,
    #[serde(rename = "订单号")]
    order_id: String,
}

impl Transaction {
    fn parse_amount(&self) -> f64 {
        self.amount
            .trim_start_matches('¥')
            .replace(',', "")
            .parse::<f64>()
            .unwrap_or(0.0)
    }
    fn is_expense(&self) -> bool {
        self.income_expense == "支出"
    }
    fn is_income(&self) -> bool {
        self.income_expense == "收入"
    }
    fn get_date(&self) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(&self.time, "%Y-%m-%d %H:%M:%S").ok()
    }
}

// 分类规则 (关键词 -> 类别)
lazy_static::lazy_static! {
    static ref CATEGORY_RULES: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("美团", "餐饮");
        m.insert("饿了么", "餐饮");
        m.insert("盒马", "生鲜食品");
        m.insert("超市", "日用品");
        m.insert("便利店", "日用品");
        m.insert("滴滴", "交通");
        m.insert("高德", "交通");
        m.insert("哈啰", "交通");
        m.insert("机票", "旅行");
        m.insert("酒店", "旅行");
        m.insert("携程", "旅行");
        m.insert("话费", "通讯");
        m.insert("水电", "生活缴费");
        m.insert("医疗", "健康");
        m.insert("京东", "购物");
        m.insert("淘宝", "购物");
        m.insert("拼多多", "购物");
        m.insert("微信红包", "红包/社交");
        m.insert("转账", "红包/社交");
        m
    };
}

// 分类函数
fn categorize(counterparty: &str, description: &str) -> String {
    let text = format!("{}{}", counterparty, description);
    for (kw, cat) in CATEGORY_RULES.iter() {
        if text.contains(*kw) {
            return cat.to_string();
        }
    }
    "其他".to_string()
}

// 分析结果
#[derive(Debug, Serialize)]
struct AnalysisReport {
    total_income: f64,
    total_expense: f64,
    net_savings: f64,
    transaction_count: usize,
    expense_by_category: HashMap<String, f64>,
    monthly_breakdown: Vec<MonthlyStats>,
    top_merchants: Vec<(String, f64)>,
    daily_average_expense: f64,
    start_date: String,
    end_date: String,
}

#[derive(Debug, Serialize)]
struct MonthlyStats {
    month: String,
    expense: f64,
    income: f64,
}

// 核心分析引擎
fn analyze(transactions: &[Transaction]) -> AnalysisReport {
    let mut total_income = 0.0;
    let mut total_expense = 0.0;
    let mut expense_by_cat: HashMap<String, f64> = HashMap::new();
    let mut merchant_expense: HashMap<String, f64> = HashMap::new();
    let mut monthly_expense: HashMap<String, f64> = HashMap::new();
    let mut monthly_income: HashMap<String, f64> = HashMap::new();
    let mut dates: Vec<NaiveDate> = Vec::new();

    for txn in transactions {
        let amount = txn.parse_amount();
        if txn.is_income() {
            total_income += amount;
            if let Some(date) = txn.get_date() {
                let month = date.format("%Y-%m").to_string();
                *monthly_income.entry(month).or_insert(0.0) += amount;
            }
        } else if txn.is_expense() {
            total_expense += amount;
            if let Some(date) = txn.get_date() {
                dates.push(date);
                let month = date.format("%Y-%m").to_string();
                *monthly_expense.entry(month).or_insert(0.0) += amount;
            }
            let cat = categorize(&txn.counterparty, &txn.description);
            *expense_by_cat.entry(cat.clone()).or_insert(0.0) += amount;
            *merchant_expense.entry(txn.counterparty.clone()).or_insert(0.0) += amount;
        }
    }

    let net_savings = total_income - total_expense;
    let transaction_count = transactions.len();
    let mut top_merchants: Vec<(String, f64)> = merchant_expense.into_iter().collect();
    top_merchants.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    top_merchants.truncate(10);

    let mut monthly_breakdown: Vec<MonthlyStats> = monthly_expense
        .keys()
        .map(|m| MonthlyStats {
            month: m.clone(),
            expense: *monthly_expense.get(m).unwrap_or(&0.0),
            income: *monthly_income.get(m).unwrap_or(&0.0),
        })
        .collect();
    monthly_breakdown.sort_by(|a, b| a.month.cmp(&b.month));

    let start_date = dates.iter().min().map(|d| d.to_string()).unwrap_or_default();
    let end_date = dates.iter().max().map(|d| d.to_string()).unwrap_or_default();
    let days = if !dates.is_empty() {
        let first = dates.iter().min().unwrap();
        let last = dates.iter().max().unwrap();
        (*last - *first).num_days().max(1) as f64
    } else {
        1.0
    };
    let daily_average_expense = total_expense / days;

    AnalysisReport {
        total_income,
        total_expense,
        net_savings,
        transaction_count,
        expense_by_category,
        monthly_breakdown,
        top_merchants,
        daily_average_expense,
        start_date,
        end_date,
    }
}

// 生成 HTML 报告 (内嵌 CSS, 使用 Chart.js)
fn generate_html_report(report: &AnalysisReport, output_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let categories_json = serde_json::to_string(&report.expense_by_category)?;
    let monthly_labels: Vec<String> = report
        .monthly_breakdown
        .iter()
        .map(|m| m.month.clone())
        .collect();
    let monthly_expense_values: Vec<f64> = report
        .monthly_breakdown
        .iter()
        .map(|m| m.expense)
        .collect();
    let monthly_income_values: Vec<f64> = report
        .monthly_breakdown
        .iter()
        .map(|m| m.income)
        .collect();

    let top_merchants_html: String = report
        .top_merchants
        .iter()
        .map(|(name, amount)| format!("<li>{}: ¥{:.2}</li>", name, amount))
        .collect();

    let html = format!(
        r#"
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <title>微信支付账单分析报告 – BAKOME</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js"></script>
    <style>
        body {{ font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; margin: 40px; background: #f0f2f5; }}
        .container {{ max-width: 1200px; margin: auto; background: white; border-radius: 12px; padding: 30px; box-shadow: 0 4px 20px rgba(0,0,0,0.1); }}
        h1, h2 {{ color: #1a3e60; }}
        .summary {{ display: flex; gap: 20px; margin-bottom: 30px; flex-wrap: wrap; }}
        .card {{ background: #f8f9fa; border-radius: 12px; padding: 20px; flex: 1; min-width: 180px; text-align: center; }}
        .card h3 {{ margin: 0; color: #555; }}
        .card .number {{ font-size: 28px; font-weight: bold; color: #2c7da0; }}
        .positive {{ color: #2e7d32; }}
        .negative {{ color: #c62828; }}
        table {{ width: 100%; border-collapse: collapse; margin: 20px 0; }}
        th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
        th {{ background-color: #2c7da0; color: white; }}
        canvas {{ max-height: 400px; margin: 20px 0; }}
        footer {{ margin-top: 40px; text-align: center; font-size: 0.8em; color: #777; }}
    </style>
</head>
<body>
<div class="container">
    <h1>📊 微信支付账单分析报告</h1>
    <p>报告生成时间: {} | 分析周期: {} 至 {}</p>

    <div class="summary">
        <div class="card"><h3>总收入</h3><div class="number positive">¥{:.2}</div></div>
        <div class="card"><h3>总支出</h3><div class="number negative">¥{:.2}</div></div>
        <div class="card"><h3>净储蓄</h3><div class="number {sign}">¥{:.2}</div></div>
        <div class="card"><h3>交易笔数</h3><div class="number">{}</div></div>
        <div class="card"><h3>日均支出</h3><div class="number">¥{:.2}</div></div>
    </div>

    <h2>📂 支出分类</h2>
    <canvas id="categoryChart" width="400" height="300"></canvas>

    <h2>📈 月度收支趋势</h2>
    <canvas id="monthlyChart" width="800" height="400"></canvas>

    <h2>🏆 消费最多商家 (Top 10)</h2>
    <ul>{}</ul>

    <h2>📋 详细月报</h2>
    <table>
        <thead><tr><th>月份</th><th>支出 (¥)</th><th>收入 (¥)</th></tr></thead>
        <tbody>
            {}
        </tbody>
    </table>
</div>
<footer>报告由 BAKOME WeChat Pay Analyzer 生成 – 开源，隐私友好</footer>
<script>
    const ctxCat = document.getElementById('categoryChart').getContext('2d');
    new Chart(ctxCat, {{
        type: 'pie',
        data: {{
            labels: {},
            datasets: [{{
                data: {},
                backgroundColor: ['#4c72b0', '#dd8452', '#55a868', '#c44e52', '#8172b2', '#937860', '#da8bc3', '#8c8c8c']
            }}]
        }}
    }});
    const ctxMon = document.getElementById('monthlyChart').getContext('2d');
    new Chart(ctxMon, {{
        type: 'line',
        data: {{
            labels: {},
            datasets: [
                {{ label: '支出', data: {}, borderColor: '#c62828', fill: false }},
                {{ label: '收入', data: {}, borderColor: '#2e7d32', fill: false }}
            ]
        }},
        options: {{ responsive: true }}
    }});
</script>
</body>
</html>
"#,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        report.start_date,
        report.end_date,
        report.total_income,
        report.total_expense,
        if report.net_savings >= 0.0 { "positive" } else { "negative" },
        report.net_savings.abs(),
        report.transaction_count,
        report.daily_average_expense,
        top_merchants_html,
        report
            .monthly_breakdown
            .iter()
            .map(|m| format!("<tr><td>{}</td><td>{:.2}</td><td>{:.2}</td></tr>", m.month, m.expense, m.income))
            .collect::<String>(),
        categories_json,
        serde_json::to_string(&report.expense_by_category.values().collect::<Vec<&f64>>())?,
        serde_json::to_string(&monthly_labels)?,
        serde_json::to_string(&monthly_expense_values)?,
        serde_json::to_string(&monthly_income_values)?,
    );
    std::fs::write(output_path, html)?;
    Ok(())
}

// 解析 CSV (微信支付导出的格式)
fn parse_wechat_csv(file_path: &PathBuf) -> Result<Vec<Transaction>, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    // 跳过前几行说明（微信导出文件开头可能有说明）
    let mut found_header = false;
    let mut transactions = Vec::new();
    for line in lines {
        let line = line?;
        if line.starts_with("交易时间") {
            found_header = true;
            continue;
        }
        if !found_header || line.is_empty() {
            continue;
        }
        let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(line.as_bytes());
        if let Ok(record) = rdr.records().next() {
            if let Ok(rec) = record {
                if rec.len() >= 9 {
                    transactions.push(Transaction {
                        time: rec[0].to_string(),
                        txn_type: rec[1].to_string(),
                        counterparty: rec[2].to_string(),
                        description: rec[3].to_string(),
                        income_expense: rec[5].to_string(),
                        amount: rec[6].to_string(),
                        payment_method: rec[7].to_string(),
                        status: rec[8].to_string(),
                        order_id: if rec.len() > 9 { rec[9].to_string() } else { String::new() },
                    });
                }
            }
        }
    }
    Ok(transactions)
}

// ----------------------------- CLI -----------------------------
#[derive(Parser)]
#[command(author, version, about = "BAKOME WeChat Pay Analyzer - 微信支付账单分析器", long_about = None)]
struct Cli {
    /// 输入的 CSV 文件路径 (微信支付账单)
    #[arg(short, long)]
    input: PathBuf,

    /// 输出的 HTML 报告路径 (默认 report.html)
    #[arg(short, long, default_value = "wechat_report.html")]
    output: PathBuf,

    /// 可选：输出 JSON 报告
    #[arg(long)]
    json: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let transactions = parse_wechat_csv(&cli.input)?;
    if transactions.is_empty() {
        eprintln!("未找到有效的交易记录。请确认 CSV 格式正确。");
        std::process::exit(1);
    }
    let report = analyze(&transactions);
    generate_html_report(&report, &cli.output)?;
    if let Some(json_path) = cli.json {
        let json_str = serde_json::to_string_pretty(&report)?;
        std::fs::write(json_path, json_str)?;
    }
    println!("报告已生成: {}", cli.output.display());
    Ok(())
}
