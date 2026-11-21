use std::fs::File;
use std::{collections::VecDeque, ops::RangeInclusive};
// use std::ops::Deref;

use csv::Reader;
use eframe::egui::{self, Key};
use egui::Theme;
use egui_live::{Sample, Samples};
use egui_plot::{Corner, Line, Plot, Polygon, VLine};
use egui_svg::EguiSvg;
use itertools::Itertools;

struct App {
    samples: Samples,
    pre_samples: Samples,
    // change: Samples,
}

impl App {
    fn new(
        samples: Samples,
        pre_samples: Samples,
        /* change: Samples , */
    ) -> Self {
        Self {
            samples,
            pre_samples,
            // change,
        }
    }
}

struct LinearRegression {
    slope: f64,
    height: f64,
}
impl LinearRegression {
    pub fn from_samples(samples: &Samples) -> Self {
        struct RegressionAccumulators {
            x: f64,
            y: f64,
            xy: f64,
            x_2: f64,
        }
        // TODO: Use actual time stamps instead of indices
        let RegressionAccumulators { x, y, xy, x_2 } = samples.iter().fold(
            RegressionAccumulators {
                x: 0.0,
                y: 0.0,
                xy: 0.0,
                x_2: 0.0,
            },
            |accumulator, &Sample { value: y, time: x }| RegressionAccumulators {
                x: accumulator.x + x as f64,
                y: accumulator.y + y,
                xy: accumulator.xy + (x as f64 * y),
                x_2: accumulator.x_2 + (x as f64).powi(2),
            },
        );
        let n = samples.len() as f64;
        let slope = (y * x - (n * xy)) / (x * x - (n * x_2));
        let height = (y - (x * slope)) / n;
        // println!("{height} + {slope}x");
        Self { slope, height }
    }
    pub fn predict_y_at_x(&self, x: f64) -> f64 {
        self.slope * x + self.height
    }
}

fn new_span(
    name: impl ToString,
    span: RangeInclusive<f64>,
    size: RangeInclusive<f64>,
) -> Polygon<'static> {
    Polygon::new(vec![
        [*span.start(), *size.start()],
        [*span.start(), *size.end()],
        [*span.end(), *size.end()],
        [*span.end(), *size.start()],
    ])
    .name(name)
}

impl App {
    fn draw(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let samples = &mut self.samples;
            let pre_samples = &mut self.pre_samples;
            // let change = &mut self.change;
            let offset = samples.len().saturating_sub(1_000_000);
            let pre_offset = pre_samples.len().saturating_sub(1_000_000);
            // let change_offset = change.len().saturating_sub(1_000_000);
            let samples = samples.clone().split_off(offset);
            let pre_samples = pre_samples.clone().split_off(pre_offset);
            // let change = change.clone().split_off(change_offset);
            let regression = LinearRegression::from_samples(&samples);
            // let pre_regression = LinearRegression::from_samples(&pre_samples);

            let samples = samples
                .into_iter()
                .map(|Sample { time, value }| [time as f64, value])
                .collect_vec();

            let pre_samples = pre_samples
                .into_iter()
                .map(|Sample { time, value }| [time as f64, value])
                .collect_vec();

            /*let change = change
            .into_iter()
            .map(|Sample { time, value }| [time as f64, value])
            .collect_vec();*/
            Plot::new("live_plot")
                .legend(egui_plot::Legend::default().position(Corner::LeftTop))
                // .include_y(-2.0)
                // .include_y(2.0)
                .y_axis_label("") // "Voltaje (mV)")
                .x_axis_label("Tiempo (μs)")
                .allow_zoom(true)
                .allow_drag(true)
                .show(ui, |plot_ui| {
                    if plot_ui.response().clicked() {
                        println!("Clicked at: {:?}", plot_ui.pointer_coordinate());
                    }
                    // plot_ui.line(Line::new("Cambio Capa 1", change));
                    plot_ui.vline(
                        VLine::new(405330000)
                            .name("Primeros valores atípicos")
                            .style(egui_plot::LineStyle::Dotted { spacing: 5.0 }),
                    );
                    plot_ui.vline(VLine::new(757922000).name("Primer salto abrupto"));
                    plot_ui.polygon(new_span(
                        "Aumento en comportamiento errático",
                        1174124000.0..=1337040000.0,
                        1245.155356500963..=1824.9181931636215,
                    ));
                    plot_ui.polygon(new_span(
                        "Secado del sensor",
                        1345600000.0..=1379516000.0,
                        1704.6579814727043..=1353.5051562594435,
                    ));
                    plot_ui.polygon(new_span(
                        "Corto circuito",
                        2518832000.0..=2644900000.0,
                        2800.0..=3153.1634168729142,
                    ));
                    plot_ui.vline(VLine::new(2644900000.0).name("Retirada del sensor"));

                    plot_ui.vline(VLine::new(2057708386.1702898).name("Pequeño salto"));

                    plot_ui.line(
                        Line::new(
                            samples
                                .iter()
                                .map(|&[x, _]| [x, regression.predict_y_at_x(x)])
                                .collect_vec(),
                        )
                        .name("Regresión linear"),
                    );
                    plot_ui.line(Line::new(pre_samples.clone()).name("Capa 1"));
                    plot_ui.line(Line::new(samples).name("Capa 3"));
                    /*plot_ui.line(Line::new(
                        "Primera regresión linear",
                        pre_samples
                            .into_iter()
                            .map(|[x, _]| [x, pre_regression.predict_y_at_x(x)])
                            .collect_vec(),
                    ));*/
                });
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(Key::E)) {
            std::fs::write(
                "graph.svg",
                EguiSvg::new(680, 750).run(|ctx| {
                    // ctx.set_pixels_per_point(30.0);
                    ctx.set_theme(Theme::Light);
                    self.draw(ctx);
                }),
            )
            .unwrap();
        }

        self.draw(ctx);

        // Redraw continuously for real-time updates.
        ctx.request_repaint();
    }
}

fn choose_file(name: &'static str) -> Reader<File> {
    Reader::from_reader(File::open(name).unwrap())
}

fn main() -> eframe::Result {
    let samples: Samples = VecDeque::from_iter(
        choose_file("SecondSampleReadings.csv")
            .records()
            .flat_map(|record| record.ok())
            .map(|record| record.deserialize(None))
            .flat_map(|record| record.ok()),
    );
    let offset = samples[0].time;
    let mut samples = samples.into_iter();
    samples.next_back(); // last element is almost always cut off
    let samples: Samples = samples
        .map(|Sample { time, value }| Sample {
            time: (time - offset), // * (2714889524 - 15481308) / (2569662478 - 318672035),
            value, // : (value - 700.0) * (1813.0 - 700.0) / (1489.0 - 700.0) + 700.0,
        })
        .collect();

    for [
        &Sample {
            time: time_a,
            value: value_a,
        },
        &Sample {
            time: time_b,
            value: value_b,
        },
    ] in samples.iter().array_windows()
    {
        let difference = value_b - value_a;
        if difference.abs() > 2.5 {
            println!("Found a difference of: {difference} from {time_a} μs to {time_b} μs.");
        }
    }
    let pre_samples: Samples = VecDeque::from_iter(
        choose_file("FirstSampleReadings.csv")
            .records()
            .flat_map(|record| record.ok())
            .map(|record| record.deserialize(None))
            .flat_map(|record| record.ok()),
    );
    let offset = pre_samples[0].time;
    let mut pre_samples = pre_samples.into_iter();
    pre_samples.next_back(); // last element is almost always cut off
    let pre_samples: Samples = pre_samples
        .map(|Sample { time, value }| Sample {
            time: time - offset,
            value,
        })
        .collect();
    /* let change = pre_samples
    .iter()
    .array_windows()
    .map(
        |[
            &Sample {
                time: time_a,
                value: value_a,
            },
            &Sample {
                time: time_b,
                value: value_b,
            },
        ]| Sample {
            time: (time_a + time_b) / 2,
            value: value_b - value_a,
        },
    )
    .collect();*/
    let paused: bool = true;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "egui_plot live",
        options,
        Box::new(|_cc| {
            Ok(Box::new(App::new(
                samples,
                pre_samples,
                /* change, */
            )))
        }),
    )
}
