use eframe::egui;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 대시보드 상태
#[derive(Default)]
pub struct DashboardState {
    pub balance: f64,
    pub total_asset: f64,
    pub position_ticker: Option<String>,
    pub position_amount: f64,
    pub position_avg_price: f64,
    pub position_current_price: f64,
    pub position_profit_rate: f64,
    pub recent_trades: Vec<TradeInfo>,
    pub monitoring_markets: Vec<String>,
    pub bot_running: bool,
}

#[derive(Clone)]
pub struct TradeInfo {
    pub ticker: String,
    pub action: String, // "BUY" or "SELL"
    pub price: f64,
    pub amount: f64,
    pub profit: Option<f64>,
    pub time: String,
}

pub struct TradingDashboard {
    state: Arc<RwLock<DashboardState>>,
}

impl TradingDashboard {
    pub fn new(state: Arc<RwLock<DashboardState>>) -> Self {
        Self { state }
    }
}

impl eframe::App for TradingDashboard {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1초마다 UI 업데이트
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        let state = self.state.clone();
        let rt = tokio::runtime::Handle::current();

        // 상태 읽기
        let state_data = rt.block_on(async {
            state.read().await.clone()
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🚀 Upbit 자동매매 봇");
            ui.separator();

            // 상태 표시
            ui.horizontal(|ui| {
                ui.label("봇 상태:");
                if state_data.bot_running {
                    ui.colored_label(egui::Color32::GREEN, "● 실행 중");
                } else {
                    ui.colored_label(egui::Color32::RED, "● 중지됨");
                }
            });
            ui.add_space(10.0);

            // 잔액 정보
            ui.group(|ui| {
                ui.heading("💰 자산 현황");
                ui.horizontal(|ui| {
                    ui.label("KRW 잔액:");
                    ui.label(format!("{:.0}원", state_data.balance));
                });
                ui.horizontal(|ui| {
                    ui.label("총 자산:");
                    ui.label(format!("{:.0}원", state_data.total_asset));
                });
            });
            ui.add_space(10.0);

            // 포지션 정보
            ui.group(|ui| {
                ui.heading("📊 현재 포지션");
                if let Some(ticker) = &state_data.position_ticker {
                    ui.horizontal(|ui| {
                        ui.label("코인:");
                        ui.strong(ticker);
                    });
                    ui.horizontal(|ui| {
                        ui.label("수량:");
                        ui.label(format!("{:.8}", state_data.position_amount));
                    });
                    ui.horizontal(|ui| {
                        ui.label("평균 매수가:");
                        ui.label(format!("{:.0}원", state_data.position_avg_price));
                    });
                    ui.horizontal(|ui| {
                        ui.label("현재가:");
                        ui.label(format!("{:.0}원", state_data.position_current_price));
                    });
                    ui.horizontal(|ui| {
                        ui.label("수익률:");
                        let color = if state_data.position_profit_rate >= 0.0 {
                            egui::Color32::GREEN
                        } else {
                            egui::Color32::RED
                        };
                        ui.colored_label(color, format!("{:+.2}%", state_data.position_profit_rate));
                    });
                } else {
                    ui.label("포지션 없음");
                }
            });
            ui.add_space(10.0);

            // 최근 거래
            ui.group(|ui| {
                ui.heading("📝 최근 거래");
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        if state_data.recent_trades.is_empty() {
                            ui.label("거래 내역 없음");
                        } else {
                            for trade in state_data.recent_trades.iter().rev().take(10) {
                                ui.horizontal(|ui| {
                                    let color = if trade.action == "BUY" {
                                        egui::Color32::LIGHT_BLUE
                                    } else {
                                        egui::Color32::LIGHT_RED
                                    };
                                    ui.colored_label(color, &trade.action);
                                    ui.label(&trade.ticker);
                                    ui.label(format!("{:.0}원", trade.price));
                                    if let Some(profit) = trade.profit {
                                        let profit_color = if profit >= 0.0 {
                                            egui::Color32::GREEN
                                        } else {
                                            egui::Color32::RED
                                        };
                                        ui.colored_label(profit_color, format!("{:+.2}%", profit));
                                    }
                                    ui.label(&trade.time);
                                });
                            }
                        }
                    });
            });
            ui.add_space(10.0);

            // 모니터링 중인 마켓
            ui.group(|ui| {
                ui.heading("👁 모니터링 중");
                ui.label(format!("총 {} 개 KRW 마켓", state_data.monitoring_markets.len()));

                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        let columns = 4;
                        let mut markets = state_data.monitoring_markets.iter();

                        while markets.len() > 0 {
                            ui.horizontal(|ui| {
                                for _ in 0..columns {
                                    if let Some(market) = markets.next() {
                                        ui.label(market);
                                    }
                                }
                            });
                        }
                    });
            });
        });
    }
}

impl Clone for DashboardState {
    fn clone(&self) -> Self {
        Self {
            balance: self.balance,
            total_asset: self.total_asset,
            position_ticker: self.position_ticker.clone(),
            position_amount: self.position_amount,
            position_avg_price: self.position_avg_price,
            position_current_price: self.position_current_price,
            position_profit_rate: self.position_profit_rate,
            recent_trades: self.recent_trades.clone(),
            monitoring_markets: self.monitoring_markets.clone(),
            bot_running: self.bot_running,
        }
    }
}

/// GUI 실행 함수
pub fn run_dashboard(state: Arc<RwLock<DashboardState>>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Upbit 자동매매 봇"),
        ..Default::default()
    };

    eframe::run_native(
        "Upbit Trading Bot",
        options,
        Box::new(|_cc| Box::new(TradingDashboard::new(state))),
    )
}
