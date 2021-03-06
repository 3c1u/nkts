pub mod animation;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::utils::easing::Easing;

use animation::{Animation, AnimationType};

#[derive(Clone, Default)]
pub struct LayerModel {
    // Layer number.
    pub layer_no: i32,
    // S25 filename and entries
    pub filename: Option<PathBuf>,
    pub entries: Vec<i32>,
    // layer property
    pub origin: (f64, f64),
    pub opacity: f32,
    pub blur_radius: (i32, i32),
    // TODO: overlay
    pub overlay: Option<PathBuf>,
    pub overlay_entries: Vec<i32>,
    pub overlay_rate: f32,
    // inner state
    command_queue: VecDeque<LayerCommand>,
    state: LayerState,
    animations: Vec<Animation>,
    finalize_mode: bool,
}

#[derive(Clone, PartialEq)]
pub enum LayerState {
    Idle,
    WaitDraw,
    Timer { wait_until: Instant },
}

impl Default for LayerState {
    fn default() -> Self {
        Self::Idle
    }
}

impl AnimationType {
    pub fn interpolate(&self, other: &AnimationType, t: f64) -> AnimationType {
        // clamp to [0, 1]
        let t = 1.0 - t.max(0.0).min(1.0);

        match (self, other) {
            (&AnimationType::MoveTo(x_from, y_from), &AnimationType::MoveTo(x_to, y_to)) => {
                let x = x_to + ((x_from - x_to) * t);
                let y = y_to + ((y_from - y_to) * t);

                AnimationType::MoveTo(x, y)
            }
            (&AnimationType::Opacity(from), &AnimationType::Opacity(to)) => {
                let opacity = to + ((from - to) as f64 * t) as f32;

                AnimationType::Opacity(opacity)
            }
            _ => unreachable!("animation type should match"),
        }
    }
}

#[derive(Clone)]
pub enum LayerCommand {
    LayerClear,
    LayerLoadS25(PathBuf),
    LayerLoadEntries(Vec<i32>),
    LayerDelay(Duration),
    LayerMoveTo(f64, f64),
    LayerOpacity(f32),
    LayerBlur(i32, i32),
    LayerWaitDraw,
    LayerAnimate {
        duration: Duration,
        to: AnimationType,
        easing: Easing,
        then: Vec<LayerCommand>,
    },
}

impl LayerModel {
    pub fn new(layer_no: i32) -> Self {
        Self {
            layer_no,
            ..Default::default()
        }
    }

    fn tick(&mut self, now: Instant) {
        // animate
        let animations = std::mem::replace(&mut self.animations, vec![]);

        self.animations = animations
            .into_iter()
            .filter_map(|a| {
                if self.finalize_mode || (a.start_time + a.duration) < now {
                    match &a.to {
                        &AnimationType::MoveTo(x, y) => {
                            self.origin = (x, y);
                        }
                        &AnimationType::Opacity(opacity) => {
                            self.opacity = opacity;
                        }
                        _ => unreachable!("all animation should be transformed to **To format"),
                    }

                    self.state = LayerState::Idle;

                    for c in a.then {
                        self.command_queue.push_back(c);
                    }

                    return None;
                } else if now < a.start_time {
                    return Some(a);
                }

                let delta_time = (now - a.start_time).as_secs_f64();
                let t = delta_time * a.rate;
                let res = a.from.interpolate(&a.to, a.easing.apply(t));

                match res {
                    AnimationType::MoveTo(x, y) => {
                        self.origin = (x, y);
                    }
                    AnimationType::Opacity(opacity) => {
                        self.opacity = opacity;
                    }
                    _ => unreachable!("all animation should be transformed to **To format"),
                }

                Some(a)
            })
            .collect();

        // state
        match &self.state {
            LayerState::Idle => {
                // do nothing
            }
            LayerState::WaitDraw => {
                if self.finalize_mode {
                    self.state = LayerState::Idle;
                }
            }
            LayerState::Timer { wait_until } => {
                // foce finalize
                if self.finalize_mode {
                    self.state = LayerState::Idle;
                    return;
                }

                // check timer
                if now < *wait_until {
                    return;
                }

                self.state = LayerState::Idle;
            }
        }
    }

    pub fn poll(&mut self, now: Instant) {
        // set to idle (workaround for slow texture loading)
        if self.state == LayerState::WaitDraw {
            self.state = LayerState::Idle;
        }

        loop {
            // generate state
            self.update(now);

            // proceed current event
            self.tick(now);

            if (!self.finalize_mode && LayerState::Idle != self.state)
                || self.command_queue.is_empty()
            {
                break;
            }
        }

        self.finalize_mode = false;
    }

    pub fn send(&mut self, command: LayerCommand) {
        self.command_queue.push_back(command);
    }

    pub fn finalize(&mut self) {
        self.finalize_mode = true;
    }

    pub fn update(&mut self, now: Instant) {
        let _layer = self.layer_no;

        // if the layer is not ready, ignore
        // and let the poller finish all the event
        if LayerState::Idle != self.state {
            return;
        }

        // process a command
        match self.command_queue.pop_front() {
            Some(LayerCommand::LayerWaitDraw) => {
                self.state = LayerState::WaitDraw;
            }
            Some(LayerCommand::LayerClear) => {
                self.filename = None;
                self.entries = vec![];
                // TODO: clear layer
            }
            Some(LayerCommand::LayerLoadS25(filename)) => {
                self.filename = Some(filename.clone());
                // TODO: load S25 image
            }
            Some(LayerCommand::LayerLoadEntries(entries)) => {
                self.entries = entries.clone();
                // TODO: load S25 entries
            }
            Some(LayerCommand::LayerMoveTo(x, y)) => {
                self.origin = (x, y);
            }
            Some(LayerCommand::LayerOpacity(opacity)) => {
                self.opacity = opacity;
            }
            Some(LayerCommand::LayerBlur(x, y)) => {
                self.blur_radius = (x, y);
            }
            Some(LayerCommand::LayerDelay(t)) => {
                if self.finalize_mode {
                    self.state = LayerState::Idle;
                } else {
                    self.state = LayerState::Timer {
                        wait_until: now + t,
                    };
                }
            }
            Some(LayerCommand::LayerAnimate {
                duration,
                to,
                easing,
                then,
            }) => {
                let (initial_state, to) = match &to {
                    &AnimationType::MoveTo(_, _) => {
                        let (x_from, y_from) = self.origin;
                        (AnimationType::MoveTo(x_from, y_from), to)
                    }
                    &AnimationType::MoveBy(dx, dy) => {
                        let (x_from, y_from) = self.origin;

                        // translate MoveBy to MoveTo
                        (
                            AnimationType::MoveTo(x_from, y_from),
                            AnimationType::MoveTo(x_from + dx, y_from + dy),
                        )
                    }
                    &AnimationType::Opacity(opacity) => {
                        let opacity_from = self.opacity;
                        self.opacity = opacity;
                        (AnimationType::Opacity(opacity_from), to)
                    }
                };

                self.animations.push(Animation {
                    start_time: now,
                    duration,
                    rate: 1.0 / duration.as_secs_f64(),
                    from: initial_state,
                    to,
                    easing,
                    then,
                });
            }
            _ => {
                // ignore
            }
        }
    }
}
