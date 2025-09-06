use egui::Ui;
use egui_plot::{Bar, BarChart, Plot};
use engine::{parser::Trades, utils::trade::TradeSummary};
use polars::frame::DataFrame;

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

    // Show the bar chart on the left
    ui.horizontal(|ui| {
        // Here you would create the bar chart using the summary.bar_chart_data
        // Create some bars
        let bars = summary
            .bar_chart_data
            .iter()
            .enumerate()
            .map(|(i, &value)| {
                let color = if value >= 0.0 {
                    egui::Color32::GREEN
                } else {
                    egui::Color32::RED
                };
                Bar::new(i as f64, value).width(0.8).fill(color)
            })
            .collect();

        // Turn into a chart
        let chart = BarChart::new("Example Bars", bars);

        // Show it inside a plot with width 25% of available width
        let available_width = ui.available_width();
        let mut available_height = ui.available_height();
        if available_height < 300.0 {
            available_height = 300.0;
        }
        Plot::new("bar_chart_example")
            // .view_aspect(2.0)
            .width(available_width * 0.40)
            .height(available_height * 0.75)
            .show_grid(false)
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(chart);
            });

        // Show the metrics on the right
        ui.vertical(|ui| {
            ui.heading(format!("Trade Metrics for {}", trades.over_frame));
            ui.separator();
            ui.label(format!("Total Trades: {}", summary.total_trades));
            ui.label(format!("Win Rate: {:.2}%", summary.win_rate));
            ui.label(format!(
                "Average Win per $1000: {}",
                summary.avg_win_per_1000
            ));
            ui.label(format!(
                "Average Loss per $1000: {}",
                summary.avg_loss_per_1000
            ));
        });
    });
}
