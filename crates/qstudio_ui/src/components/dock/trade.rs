use engine::controllers::Output;

use crossbeam_channel::Receiver;
use egui::Ui;
use egui::{FontId, RichText};
use egui_extras::{Column, TableBuilder};
use egui_plot::{Bar, BarChart, Plot};
use engine::{parser::Trades, utils::trade::TradeSummary};
use polars::frame::DataFrame;

#[derive(Debug, Clone)]
pub struct TradeSummaryUi {
    pub output: Option<Output>,
    pub listen: Receiver<Output>,
    pub summary: Option<TradeSummary>,
    pub trades: Option<Trades>,
}

impl TradeSummaryUi {
    pub fn new(listen: Receiver<Output>) -> Self {
        TradeSummaryUi {
            output: None,
            listen,
            summary: None,
            trades: None,
        }
    }

    fn pump_snapshots(&mut self, ctx: &egui::Context) {
        let mut update = false;
        if let Ok(output) = self.listen.try_recv() {
            self.output = Some(output);
            if let Some(output) = &self.output {
                if let Some(trades) = output.get_trades() {
                    let summary = trades.trade_summary.clone();
                    self.summary = Some(summary);
                    self.trades = Some(trades);
                    update = true;
                }
            }
        }
        if update {
            ctx.request_repaint();
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        self.pump_snapshots(ui.ctx());
        // If we have a summary, display it
        if let (Some(summary), Some(trades)) = (&self.summary, &self.trades) {
            trade_summary_ui(ui, summary.clone(), trades.clone());
        } else {
            ui.label("No trade data available.");
        }
    }
}

pub fn trade_summary_ui(ui: &mut Ui, summary: TradeSummary, trades: Trades) {
    // Show trade summary information
    // on the left side of the screen will be a bar chart of all trades found
    // it will be in the negatives and red if the trade hit the limit
    // or it will be positive and green if the trade hit the exit
    // on the right side of the screen will be a list of metrics
    // total trades
    // win rate
    // average win per $1000
    // average loss per $1000

    let available_width = ui.available_width();
    let mut available_height = ui.max_rect().height();

    let bars = summary
        .bar_chart_data
        .iter()
        .enumerate()
        .map(|(i, &value)| {
            let color = if value >= 0.0 {
                egui::Color32::from_rgb(100, 200, 100) // Calmer green
            } else {
                egui::Color32::from_rgb(200, 100, 100) // Calmer red
            };
            Bar::new(i as f64, value).width(0.8).fill(color)
        })
        .collect();

    // Turn into a chart
    let chart = BarChart::new("Trade Summary", bars);

    // Show the bar chart on the left
    ui.horizontal(|ui| {
        // Here you would create the bar chart using the summary.bar_chart_data
        // Create some bars

        // Show it inside a plot with width 25% of available width

        Plot::new("bar_chart_example")
            // .view_aspect(2.0)
            .width(available_width * 0.50)
            .height(available_height)
            .show_grid(false)
            .show_background(false)
            .center_y_axis(true)
            .allow_scroll(false)
            .allow_zoom(false)
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(chart);
            });

        // Show the metrics on the right
        ui.vertical(|ui| {
            ui.heading(format!("Trade Metrics for {}", trades.over_frame));
            ui.separator();

            let label_font = FontId::proportional(14.0);

            ui.add_space(8.0);
            TableBuilder::new(ui)
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
                .columns(Column::exact(200.0), 1) // First column wider
                .columns(Column::auto().at_least(120.0), 1) // Second column auto
                .body(|mut body| {
                    body.row(24.0, |mut row| {
                        row.col(|ui| {
                            ui.label(RichText::new("Total Trades:").font(label_font.clone()));
                        });
                        row.col(|ui| {
                            ui.label(
                                RichText::new(format!("{}", summary.total_trades))
                                    .font(label_font.clone()),
                            );
                        });
                    });
                    body.row(24.0, |mut row| {
                        row.col(|ui| {
                            ui.label(RichText::new("Win Rate:").font(label_font.clone()));
                        });
                        row.col(|ui| {
                            ui.label(
                                RichText::new(format!("{:.2}%", summary.win_rate))
                                    .font(label_font.clone()),
                            );
                        });
                    });
                    body.row(24.0, |mut row| {
                        row.col(|ui| {
                            ui.label(
                                RichText::new("Average Win per $1000:").font(label_font.clone()),
                            );
                        });
                        row.col(|ui| {
                            ui.label(
                                RichText::new(format!("${:.2}", summary.avg_win_per_1000))
                                    .font(label_font.clone()),
                            );
                        });
                    });
                    body.row(24.0, |mut row| {
                        row.col(|ui| {
                            ui.label(
                                RichText::new("Average Loss per $1000:").font(label_font.clone()),
                            );
                        });
                        row.col(|ui| {
                            ui.label(
                                RichText::new(format!("${:.2}", summary.avg_loss_per_1000))
                                    .font(label_font.clone()),
                            );
                        });
                    });
                });
        });
    });
}
