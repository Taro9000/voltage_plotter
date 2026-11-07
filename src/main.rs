use std::collections::VecDeque;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use eframe::egui;
use egui_plot::{BoxElem, BoxPlot, BoxSpread, Plot};
use itertools::Itertools;

type Samples = VecDeque<Sample>;

#[derive(Clone, Copy)]
struct Sample {
    time: f64,
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

    // xorshift64: tiny dependency-free random numbers in [-1, 1]
}

fn next_noise(rng: &mut u64) -> f64 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    (*rng as f64 / u64::MAX as f64) * 2.0 - 1.0
}

fn push_sample(rng: &mut u64, samples: &mut Samples) {
    let t = samples.len() as f64;
    let n = next_noise(rng);
    samples.push_back(Sample {
        time: t,
        value: (t * 0.001).sin() + (n * 0.5) + (t * 0.001 * ((1.0 + 5.0f64.sqrt()) / 2.0)).cos(),
    });
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        egui::CentralPanel::default().show(ui, |ui| {
            let samples = { self.samples.lock().unwrap().clone() };
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.paused.lock().unwrap(), "Pause");
                ui.label(format!(
                    "{} points/series | {:.0} fps",
                    samples.len(),
                    1.0 / ctx.input(|i| i.stable_dt).max(1e-6)
                ));
            });

            let range = samples.len().saturating_sub(67_000_000)..samples.len();
            let size = range.clone().count() / 42;
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
                                ((index * 2 + 1) * size) as f64 / 2.0,
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
                .include_y(-2.0)
                .include_y(2.0)
                .allow_zoom(true)
                .allow_drag(true)
                .show(ui, |plot_ui| {
                    plot_ui.box_plot(BoxPlot::new("sin(2t)", samples));
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
        loop {
            {
                if !thread_paused.lock().unwrap().deref() {
                    let mut samples = samples.lock().unwrap();
                    for _ in 0..100 {
                        push_sample(&mut rng, &mut samples);
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
