use std::time::Instant;
use std::{cmp::max, collections::VecDeque};

use eframe::egui;
use egui_plot::{BoxElem, BoxPlot, BoxSpread, HLine, Line, Plot, PlotPoints, Points};
use itertools::Itertools;
use rayon::prelude::*;

const WINDOW_POINTS: usize = 10_000;

#[derive(Clone, Copy)]
struct Sample {
    time: f64,
    value: f64,
}

struct App {
    samples: VecDeque<Sample>,
    rng: u64,
    paused: bool,
}

impl App {
    fn new() -> Self {
        Self {
            samples: VecDeque::new(),
            rng: 0x2545F4914F6CDD1D,
            paused: true,
        }
    }

    // xorshift64: tiny dependency-free random numbers in [-1, 1]
    fn next_noise(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng as f64 / u64::MAX as f64) * 2.0 - 1.0
    }

    fn push_sample(&mut self) {
        let t = self.samples.len() as f64;
        let n = self.next_noise();
        self.samples.push_back(Sample {
            time: t,
            value: (t * 0.001).sin()
                + (n * 0.5)
                + (t * 0.001 * ((1.0 + 5.0f64.sqrt()) / 2.0)).cos(),
        });
        let cutoff = t - WINDOW_POINTS as f64;
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.paused {
            for _ in 0..100 {
                self.push_sample();
            }
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.paused, "Pause");
                ui.label(format!(
                    "{} points/series | {:.0} fps",
                    self.samples.len(),
                    1.0 / ctx.input(|i| i.stable_dt).max(1e-6)
                ));
            });

            let samples = &self.samples;
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "egui_plot live",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
