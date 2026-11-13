mod driver;

use std::collections::VecDeque;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use egui_plot::{BoxElem, BoxPlot, BoxSpread, Line, Plot, PlotPoints};
use itertools::Itertools;

type Samples = VecDeque<Sample>;

struct WaveLayer {
    sin: f64,
    cos: f64,
    frequency: f64,
}

#[derive(Clone, Copy)]
struct Sample {
    time: Instant,
    value: f64,
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
        time: Instant::now(),
        value: fractalized + (noise * 42.067),
    };
    // println!("{:?}: {}", sample.time, sample.value);
    samples.push_back(sample);
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        egui::CentralPanel::default().show(ui, |ui| {
            let mut samples = { self.samples.lock().unwrap().clone() };
            let offset = samples.len().saturating_sub(100000);
            let samples = samples.split_off(offset);
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
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.paused.lock().unwrap(), "Pause");
                ui.label(format!(
                    "{} points/series | {:.0} fps",
                    samples.len(),
                    1.0 / ctx.input(|i| i.stable_dt).max(1e-6)
                ));
            });

            let range = samples.len().saturating_sub(67_000_000)..samples.len();
            let size = range.clone().count() / 67;
            let samples = samples
                .range(range)
                .chunks(size + 1)
                .into_iter()
                .enumerate()
                .flat_map(|(index, range)| {
                    let sorted = range
                        .sorted_by(|Sample { value: a, .. }, Sample { value: b, .. }| {
                            a.total_cmp(b)
                        })
                        .collect_vec();

                    if let (
                        Some(&&Sample {
                            value: lower_whisker,
                            ..
                        }),
                        Some(&&Sample {
                            value: lower_quartile,
                            ..
                        }),
                        Some(&&Sample { value: median, .. }),
                        Some(&&Sample {
                            value: upper_quartile,
                            ..
                        }),
                        Some(&&Sample {
                            value: upper_whisker,
                            ..
                        }),
                    ) = (
                        sorted.first(),
                        sorted.get(sorted.len() / 4),
                        sorted.get(sorted.len() / 2),
                        sorted.get(sorted.len() * 3 / 4),
                        sorted.get(sorted.len().saturating_sub(1)),
                    ) {
                        Some(
                            BoxElem::new(
                                ((index * 2 + 1) * size) as f64 / 2.0 + offset as f64,
                                BoxSpread::new(
                                    lower_whisker,
                                    lower_quartile,
                                    median,
                                    upper_quartile,
                                    upper_whisker,
                                ),
                            )
                            .box_width(size as f64 / 2.0),
                        )
                    } else {
                        None
                    }
                })
                .collect_vec();
            Plot::new("live_plot")
                .legend(egui_plot::Legend::default())
                // .include_y(-2.0)
                // .include_y(2.0)
                .allow_zoom(true)
                .allow_drag(true)
                .show(ui, |plot_ui| {
                    plot_ui.box_plot(BoxPlot::new("data", samples));
                    plot_ui.line(Line::new(
                        "linear regression",
                        PlotPoints::new(vec![
                            [offset as f64, offset as f64 * slope + height],
                            [n + offset as f64, (n + offset as f64) * slope + height],
                        ]),
                    ));
                });
        });

        // Redraw continuously for real-time updates.
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result {
    let samples: Arc<Mutex<Samples>> = Arc::new(Mutex::new(VecDeque::new()));
    let paused: Arc<Mutex<bool>> = Arc::new(Mutex::new(true));
    let thread_samples = samples.clone();
    let thread_paused: Arc<Mutex<bool>> = paused.clone();
    thread::spawn(move || {
        let mut rng: u64 = 0x2545F4914F6CDD1D;
        let growth = (1.0 + 5f64.sqrt()) / 2.0;
        let small_phi = (1.0 - 5f64.sqrt()) / 2.0;
        let wave_layers = (0..30)
            .map(|index| WaveLayer {
                sin: next_noise(&mut rng) * growth.powi(index / 2),
                cos: next_noise(&mut rng) * growth.powi(2 * index),
                frequency: small_phi.powi(2 * index) * 0.01,
            })
            .collect_vec();
        let mut state = driver::State::SearchingPorts;
        println!("Data collector driver starting...");
        loop {
            {
                let mut samples = samples.lock().unwrap();
                state = state.iterate(&mut samples);
                if !thread_paused.lock().unwrap().deref() {
                    for _ in 0..10 {
                        push_sample(&mut rng, &mut samples, &wave_layers);
                    }
                }
            }
            thread::sleep(Duration::from_millis(1));
        }
    });
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
