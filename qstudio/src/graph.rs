use egui::{Color32, Stroke, TextStyle};
use egui_plot::{
    uniform_grid_spacer, Bar, BarChart, BoxElem, BoxPlot, Line, Plot, PlotPoints,
    Legend, Corner,
};
use engine::parser::{DrawType, Graph};

pub struct DrawGraph(pub Graph);

impl DrawGraph {
    pub fn new(graph: Graph) -> Self {
        DrawGraph(graph)
    }

    pub fn draw(&self, ui: &mut egui::Ui) {
        let n = self.0.axis_labels.len().max(1) as f64;
        let step_size = (self
            .0
            .axis_labels
            .last()
            .and_then(|label| label.parse::<f64>().ok())
            .unwrap_or(n)
            - self
                .0
                .axis_labels
                .first()
                .and_then(|label| label.parse::<f64>().ok())
                .unwrap_or(0.0))
            / n;

        let step = move |_: egui_plot::GridInput| {
            [step_size.max(1.0), step_size.max(1.0), step_size.max(1.0)]
        };

        Plot::new(&self.0.title)
            // .show_grid([true, true])
            // .grid_spacing((step_size as f32)..=(step_size as f32))
            // .x_grid_spacer(uniform_grid_spacer(step))
            // .y_grid_spacer(uniform_grid_spacer(step))
            // .x_axis_label("Time")
            // .y_axis_label("Price")
            // .show_axes([true, true]) // Show/hide axis lines and labels
            .legend(
                Legend::default()
                    .position(Corner::RightTop)
                    .text_style(TextStyle::Small)
                    .background_alpha(0.5)
                    .follow_insertion_order(true),
            )
            .show(ui, |plot_ui| {
                for draw_type in &self.0.data {
                    match draw_type {
                        DrawType::Line(values) => {
                            let plot_points: PlotPoints = self
                                .0
                                .axis_labels
                                .iter()
                                .zip(values)
                                .map(|(label, value)| {
                                    let x = label.parse::<f64>().unwrap_or(0.0);
                                    let y = *value;
                                    [x, y]
                                })
                                .collect();

                            plot_ui.line(Line::new("line", plot_points));
                        }

                        DrawType::Bar(bars) => {
                            let bar_chart = BarChart::new(
                                "bar_chart",
                                self.0
                                    .axis_labels
                                    .iter()
                                    .zip(bars)
                                    .enumerate()
                                    .map(|(i, (label, (open, close)))| {
                                        let x = label.parse::<f64>().unwrap_or(i as f64);
                                        Bar::new(x, close - open).width(0.5)
                                    })
                                    .collect(),
                            )
                            .color(Color32::LIGHT_BLUE);
                            plot_ui.bar_chart(bar_chart);
                        }

                        DrawType::Candlestick(candles) => {
                            // BoxPlot approach
                            let elems: Vec<BoxElem> = candles
                                .iter()
                                .enumerate()
                                .filter_map(|(i, &(open, high, low, close))| {
                                    if let Some(label) = self.0.axis_labels.get(i) {
                                        if let Ok(x) = label.parse::<f64>() {
                                            let color = if close >= open {
                                                Color32::GREEN
                                            } else {
                                                Color32::RED
                                            };
                                            Some(BoxElem {
                                                name: format!("Candle {}", i),
                                                orientation: egui_plot::Orientation::Vertical,
                                                argument: x,
                                                spread: egui_plot::BoxSpread::new(
                                                    low,
                                                    high,
                                                    open,
                                                    (open + close) / 2.0,
                                                    close,
                                                ),
                                                box_width: 0.8,
                                                whisker_width: 0.2,
                                                fill: color,
                                                stroke: Stroke::new(1.0, Color32::BLACK),
                                            })
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                })
                                .collect();

                            plot_ui.box_plot(BoxPlot::new("candlestick", elems));
                        }
                    }
                }
            });
    }
}
