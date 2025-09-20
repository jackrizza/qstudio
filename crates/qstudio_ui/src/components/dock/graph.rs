use crossbeam_channel::{Receiver, Sender};
use egui::{Color32, Stroke, TextStyle};
use egui_plot::{
    Bar, BarChart, BoxElem, BoxPlot, Corner, Legend, Line, Plot, PlotPoints, PlotUi, Polygon,
};
use engine::controllers::Output;
use engine::parser::{DrawType, Graph, Trades};

#[derive(Debug, Clone)]
pub struct DrawGraphUi {
    pub output: Option<Output>,
    pub listen: Receiver<Output>,
    graph: Option<DrawGraph>,
}

impl DrawGraphUi {
    pub fn new(listen: Receiver<Output>) -> Self {
        DrawGraphUi {
            output: None,
            listen,
            graph: None,
        }
    }

    fn pump_snapshots(&mut self, ctx: &egui::Context) {
        let mut update = false;
        while let Ok(new_output) = self.listen.try_recv() {
            self.output = Some(new_output);
            update = true;
        }

        if update {
            ctx.request_repaint();
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.pump_snapshots(ui.ctx());
        if let Some(output) = &self.output {
            if let Some(graph) = output.get_graph() {
                let trades = output.get_trades();
                let draw_graph = DrawGraph::new(graph.clone(), trades);
                self.graph = Some(draw_graph);
                self.graph.as_mut().unwrap().draw(ui);
            } else {
                ui.label("No graph data available.");
            }
        } else {
            ui.label("No output data available.");
        }
    }
}

#[derive(Debug, Clone)]
pub struct DrawGraph {
    pub graph: Graph,
    pub trades: Option<Trades>,
}

impl DrawGraph {
    pub fn new(graph: Graph, trades: Option<Trades>) -> Self {
        DrawGraph { graph, trades }
    }

    pub fn draw(&self, ui: &mut egui::Ui) {
        let n = self.graph.axis_labels.len().max(1) as f64;
        let step_size = (self
            .graph
            .axis_labels
            .last()
            .and_then(|label| label.parse::<f64>().ok())
            .unwrap_or(n)
            - self
                .graph
                .axis_labels
                .first()
                .and_then(|label| label.parse::<f64>().ok())
                .unwrap_or(0.0))
            / n;

        let _step = move |_: egui_plot::GridInput| {
            [step_size.max(1.0), step_size.max(1.0), step_size.max(1.0)]
        };

        Plot::new(&self.graph.title)
            .show_grid([false, false])
            .allow_double_click_reset(true)
            .x_axis_label("Time")
            .y_axis_label("Price")
            .default_y_bounds(self.graph.min() - 25.0, self.graph.max() + 25.0)
            .show_axes([true, true]) // Show/hide axis lines and labels
            .legend(
                Legend::default()
                    .position(Corner::RightTop)
                    .text_style(TextStyle::Body)
                    .background_alpha(0.5)
                    .follow_insertion_order(true),
            )
            .show(ui, |plot_ui| {
                for draw_type in &self.graph.data {
                    match draw_type {
                        DrawType::Line(name, values) => {
                            let plot_points: PlotPoints = self
                                .graph
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
                                    .graph
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
                                    let label = self.graph.axis_labels.get(i)?;
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
                if let Some(trades) = &self.trades {
                    self.draw_in_trades(plot_ui, trades);
                }
            });
    }

    fn draw_in_trades(&self, plot_ui: &mut PlotUi, trades: &Trades) {
        // do later
        for (buy, limit) in &trades.trades_graph {
            plot_ui.polygon(
                Polygon::new("buy_trade", buy.to_vec())
                    .fill_color(egui::Color32::from_rgba_unmultiplied(
                        0,
                        200,
                        0,
                        (0.10 * 255.0) as u8, // transparent green fill
                    ))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(0, 200, 0, (0.8 * 255.0) as u8), // more visible green stroke
                    )),
            );
            plot_ui.polygon(
                Polygon::new("limit_trade", limit.to_vec())
                    .fill_color(egui::Color32::from_rgba_unmultiplied(
                        200,
                        0,
                        0,
                        (0.10 * 255.0) as u8, // transparent red fill
                    ))
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(200, 0, 0, (0.8 * 255.0) as u8), // more visible red stroke
                    )),
            );
        }
    }
}
