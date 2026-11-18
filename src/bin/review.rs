use std::collections::VecDeque;
use std::fs::File;
use std::io::{ErrorKind, Read};
// use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use csv::{Reader, Writer};
use eframe::egui;
use egui_live::{Sample, Samples, driver};
use egui_plot::{BoxElem, BoxPlot, BoxSpread, Line, Plot, PlotPoints, Points};
use itertools::Itertools;

struct WaveLayer {
    sin: f64,
    cos: f64,
    frequency: f64,
}

struct App {
    samples: Arc<Mutex<Samples>>,
    paused: Arc<Mutex<bool>>,
}

impl App {
    fn new(samples: Arc<Mutex<Samples>>, paused: Arc<Mutex<bool>>) -> Self {
        Self { samples, paused }
    }
}

// xorshift64: tiny dependency-free random numbers in [-1, 1]
fn next_noise(rng: &mut u64) -> f64 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    (*rng as f64 / u64::MAX as f64) * 2.0 - 1.0
}

#[expect(dead_code)]
fn push_sample(rng: &mut u64, samples: &mut Samples, wave_layers: &[WaveLayer]) {
    let t = samples.len() as f64;
    let noise = next_noise(rng);
    let fractalized: f64 = wave_layers
        .iter()
        .map(
            |&WaveLayer {
                 sin,
                 cos,
                 frequency,
             }| ((t * frequency).sin() * sin) + ((t * frequency).cos() * cos),
        )
        .sum();
    let sample = Sample {
        _time: samples.len() as u128,
        value: fractalized + (noise * 42.067),
    };
    // println!("{:?}: {}", sample.time, sample.value);
    samples.push_back(sample);
}

struct LinearRegression {
    slope: f64,
    height: f64,
}
impl LinearRegression {
    pub fn from_samples(samples: &Samples, offset: usize) -> Self {
        struct RegressionAccumulators {
            x: f64,
            y: f64,
            xy: f64,
            x_2: f64,
        }
        // TODO: Use actual time stamps instead of indices
        let RegressionAccumulators { x, y, xy, x_2 } = samples.iter().enumerate().fold(
            RegressionAccumulators {
                x: 0.0,
                y: 0.0,
                xy: 0.0,
                x_2: 0.0,
            },
            |accumulator, (x, &Sample { value: y, .. })| RegressionAccumulators {
                x: accumulator.x + (x + offset) as f64,
                y: accumulator.y + y,
                xy: accumulator.xy + ((x + offset) as f64 * y),
                x_2: accumulator.x_2 + (((x + offset) as f64).powi(2)),
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

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        egui::CentralPanel::default().show(ui, |ui| {
            let mut samples = { self.samples.lock().unwrap().clone() };
            let offset = samples.len().saturating_sub(1_000_000);
            let samples = samples.split_off(offset);
            let n = samples.len() as f64;
            let regression = LinearRegression::from_samples(&samples, offset);
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.paused.lock().unwrap(), "Pause");
                ui.label(format!(
                    "{} points/series | {:.0} fps",
                    samples.len(),
                    1.0 / ctx.input(|i| i.stable_dt).max(1e-6)
                ));
            });

            let samples = samples
                .into_iter()
                .enumerate()
                .map(|(index, Sample { _time, value })| [(index + offset) as f64, value])
                .collect_vec();
            Plot::new("live_plot")
                .legend(egui_plot::Legend::default())
                // .include_y(-2.0)
                // .include_y(2.0)
                .allow_zoom(true)
                .allow_drag(true)
                .show(ui, |plot_ui| {
                    plot_ui.line(Line::new("data", samples));
                    plot_ui.line(Line::new(
                        "linear regression",
                        PlotPoints::new(vec![
                            [offset as f64, regression.predict_y_at_x(offset as f64)],
                            [
                                n + offset as f64,
                                regression.predict_y_at_x(n + offset as f64),
                            ],
                        ]),
                    ));
                });
        });

        // Redraw continuously for real-time updates.
        ctx.request_repaint();
    }
}

fn choose_file() -> Reader<File> {
    Reader::from_reader(File::open("FirstSampleReadings.csv").unwrap())
}

fn main() -> eframe::Result {
    let mut file = choose_file();
    let samples: Arc<Mutex<Samples>> = Arc::new(Mutex::new(VecDeque::from_iter(
        file.records()
            .flat_map(|record| record.ok())
            .map(|record| record.deserialize(None))
            .flat_map(|record| record.ok()),
    )));
    let paused: Arc<Mutex<bool>> = Arc::new(Mutex::new(true));
    let thread_samples = samples.clone();
    let thread_paused: Arc<Mutex<bool>> = paused.clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "egui_plot live",
        options,
        Box::new(|_cc| Ok(Box::new(App::new(thread_samples, paused)))),
    )
}
