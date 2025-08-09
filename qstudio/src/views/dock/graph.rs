use egui::{Color32, Stroke, TextStyle};
use egui_plot::{
    uniform_grid_spacer, Bar, BarChart, BoxElem, BoxPlot, Corner, Legend, Line, Plot, PlotPoint,
    PlotPoints, Polygon,
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
            .show_grid([false, false])
            .allow_double_click_reset(true)
            .x_axis_label("Time")
            .y_axis_label("Price")
            .default_y_bounds(self.0.min() - 25.0, self.0.max() + 25.0)
            .show_axes([true, true]) // Show/hide axis lines and labels
            .legend(
                Legend::default()
                    .position(Corner::RightTop)
                    .text_style(TextStyle::Body)
                    .background_alpha(0.5)
                    .follow_insertion_order(true),
            )
            .show(ui, |plot_ui| {
                for draw_type in &self.0.data {
                    match draw_type {
                        DrawType::Line(name, values) => {
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

                            plot_ui.line(Line::new(name, plot_points));
                        }

                        DrawType::Bar(name, ys) => {
                            let mut bars = Vec::new();
                            for (i, y) in ys.iter().enumerate() {
                                let arg = self
                                    .0
                                    .axis_labels
                                    .get(i)
                                    .and_then(|s| s.parse::<f64>().ok())
                                    .unwrap_or(i as f64); // fallback to index if labels missing
                                                          // println!("Bar: x = {}, y = {}", arg, y);
                                let bar = Bar::new(arg, *y)
                                    .fill(Color32::BLUE)
                                    .stroke(Stroke::new(1.0, Color32::BLUE))
                                    .width(10.0); // Adjust width for visibility
                                bars.push(bar);
                            }

                            let bar_chart = BarChart::new(name, bars);
                            // .width(10.0); // Try wide first to verify visibility

                            plot_ui.bar_chart(bar_chart);
                        }

                        DrawType::Candlestick(name, candles) => {
                            let elems: Vec<BoxElem> = candles
                                .iter()
                                .enumerate()
                                .filter_map(|(i, &(open, high, low, close))| {
                                    let label = self.0.axis_labels.get(i)?;
                                    let x = label.parse::<f64>().ok()?;

                                    let color = if close >= open {
                                        Color32::GREEN
                                    } else {
                                        Color32::RED
                                    };

                                    let (lower_box, upper_box) = if open <= close {
                                        (open, close)
                                    } else {
                                        (close, open)
                                    };

                                    Some(BoxElem {
                                        name: format!("Candle {}", i),
                                        orientation: egui_plot::Orientation::Vertical,
                                        argument: x,
                                        spread: egui_plot::BoxSpread::new(
                                            low,       // lower whisker
                                            high,      // upper whisker
                                            lower_box, // box bottom
                                            (open + close) / 2.0,
                                            upper_box, // box top
                                        ),
                                        box_width: 4.0,
                                        whisker_width: 2.0,
                                        fill: color,
                                        stroke: Stroke::new(1.0, color),
                                    })
                                })
                                .collect();

                            plot_ui.box_plot(BoxPlot::new(name, elems));
                        }

                        DrawType::RedRect(_name, rects) => {
                            for &(x0, x1, price) in rects {
                                let pts: Vec<[f64; 2]> = vec![
                                    [x0, price],
                                    [x1, price],
                                    [x1, price * 1.001],
                                    [x0, price * 1.001],
                                ];

                                plot_ui.polygon(Polygon::new("red_trade", pts).fill_color(
                                    Color32::from_rgba_premultiplied(200, 50, 50, 128),
                                ));
                            }
                        }

                        DrawType::GreenRect(_name, rects) => {
                            for &(x0, x1, price) in rects {
                                let pts: Vec<[f64; 2]> = vec![
                                    [x0, price],
                                    [x1, price],
                                    [x1, price * 1.001],
                                    [x0, price * 1.001],
                                ];

                                plot_ui.polygon(Polygon::new("green_trade", pts).fill_color(
                                    Color32::from_rgba_premultiplied(50, 200, 50, 128),
                                ));
                            }
                        }
                    }
                }
            });
    }
}
