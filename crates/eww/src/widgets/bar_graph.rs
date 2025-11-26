use anyhow::{anyhow, Result};
use gtk::glib::{self, object_subclass, wrapper, Properties};
use gtk::{cairo, gdk, prelude::*, subclass::prelude::*};
use std::{cell::RefCell, collections::VecDeque};

use crate::error_handling_ctx;

wrapper! {
    pub struct BarGraph(ObjectSubclass<BarGraphPriv>)
    @extends gtk::Widget;
}

#[derive(Properties)]
#[properties(wrapper_type = BarGraph)]
pub struct BarGraphPriv {
    #[property(get, set, nick = "Value", blurb = "The value", minimum = 0f64, maximum = f64::MAX, default = 0f64)]
    value: RefCell<f64>,

    #[property(get, set, nick = "Gradiant Style", blurb = "BarGraph color gradiant style", default = "none")]
    gradiant_style: RefCell<String>,

    #[property(get, set, nick = "Maximum Value", blurb = "The Maximum Value", minimum = 0f64, maximum = f64::MAX, default = 100f64)]
    min: RefCell<f64>,

    #[property(get, set, nick = "Minumum Value", blurb = "The Minimum Value", minimum = 0f64, maximum = f64::MAX, default = 0f64)]
    max: RefCell<f64>,

    #[property(get, set, nick = "Dynamic", blurb = "If it is dynamic", default = true)]
    dynamic: RefCell<bool>,

    #[property(get, set, nick = "Time Range", blurb = "The Time Range", minimum = 0u64, maximum = u64::MAX, default = 10u64)]
    time_range: RefCell<u64>,

    #[property(get, set, nick = "Flip X", blurb = "Flip the x axis", default = true)]
    flip_x: RefCell<bool>,
    #[property(get, set, nick = "Flip Y", blurb = "Flip the y axis", default = true)]
    flip_y: RefCell<bool>,

    #[property(get, set, nick = "Radius", blurb = "Point radius", default = 1.0)]
    radius: RefCell<f64>,

    history: RefCell<VecDeque<(std::time::Instant, f64)>>,
    extra_point: RefCell<Option<(std::time::Instant, f64)>>,
    last_updated_at: RefCell<std::time::Instant>,
}

impl Default for BarGraphPriv {
    fn default() -> Self {
        Self {
            value: RefCell::new(0.0),
            radius: RefCell::new(1.0),
            gradiant_style: RefCell::new("none".to_string()),
            min: RefCell::new(0.0),
            max: RefCell::new(100.0),
            dynamic: RefCell::new(true),
            time_range: RefCell::new(10),
            flip_x: RefCell::new(true),
            flip_y: RefCell::new(true),
            history: RefCell::new(VecDeque::new()),
            extra_point: RefCell::new(None),
            last_updated_at: RefCell::new(std::time::Instant::now()),
        }
    }
}

impl BarGraphPriv {
    // Updates the history, removing points ouside the range
    fn update_history(&self, v: (std::time::Instant, f64)) {
        let mut history = self.history.borrow_mut();
        let mut last_value = self.extra_point.borrow_mut();
        let mut last_updated_at = self.last_updated_at.borrow_mut();
        *last_updated_at = std::time::Instant::now();

        while let Some(entry) = history.front() {
            if last_updated_at.duration_since(entry.0).as_millis() as u64 > *self.time_range.borrow() {
                *last_value = history.pop_front();
            } else {
                break;
            }
        }
        history.push_back(v);
    }
    /**
     * Receives normalized (0-1) coordinates `x` and `y` and convert them to the
     * point on the widget.
     */
    fn value_to_point(&self, width: f64, height: f64, x: f64, y: f64) -> (f64, f64) {
        let x = if *self.flip_x.borrow() { 1.0 - x } else { x };
        let y = if *self.flip_y.borrow() { 1.0 - y } else { y };
        (width * x, height * y)
    }
}

impl ObjectImpl for BarGraphPriv {
    fn properties() -> &'static [glib::ParamSpec] {
        Self::derived_properties()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "value" => {
                let value = value.get().unwrap();
                self.value.replace(value);
                self.update_history((std::time::Instant::now(), value));
                self.obj().queue_draw();
            }
            "radius" => {
                self.radius.replace(value.get().unwrap());
            }
            "max" => {
                self.max.replace(value.get().unwrap());
            }
            "min" => {
                self.min.replace(value.get().unwrap());
            }
            "dynamic" => {
                self.dynamic.replace(value.get().unwrap());
            }
            "time-range" => {
                self.time_range.replace(value.get().unwrap());
            }
            "gradiant-style" => {
                self.gradiant_style.replace(value.get().unwrap());
            }
            "flip-x" => {
                self.flip_x.replace(value.get().unwrap());
            }
            "flip-y" => {
                self.flip_y.replace(value.get().unwrap());
            }
            x => panic!("Tried to set inexistant property of Graph: {}", x,),
        }
    }

    fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        self.derived_property(id, pspec)
    }
}

#[object_subclass]
impl ObjectSubclass for BarGraphPriv {
    type ParentType = gtk::Bin;
    type Type = BarGraph;

    const NAME: &'static str = "Graph";

    fn class_init(klass: &mut Self::Class) {
        klass.set_css_name("bargraph");
    }
}

impl Default for BarGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl BarGraph {
    pub fn new() -> Self {
        glib::Object::new::<Self>()
    }
}

impl ContainerImpl for BarGraphPriv {
    fn add(&self, _widget: &gtk::Widget) {
        error_handling_ctx::print_error(anyhow!("Error, BarGraph widget shoudln't have any children"));
    }
}

impl BinImpl for BarGraphPriv {}
impl WidgetImpl for BarGraphPriv {
    fn preferred_width(&self) -> (i32, i32) {
        let radius = *self.radius.borrow() as i32;
        (radius, radius)
    }

    fn preferred_width_for_height(&self, height: i32) -> (i32, i32) {
        (height, height)
    }

    fn preferred_height(&self) -> (i32, i32) {
        let radius = *self.radius.borrow() as i32;
        (radius, radius)
    }

    fn preferred_height_for_width(&self, width: i32) -> (i32, i32) {
        (width, width)
    }

    fn draw(&self, cr: &cairo::Context) -> glib::Propagation {
        let res: Result<()> = (|| {
            let styles = self.obj().style_context();
            let color: gdk::RGBA = styles.color(gtk::StateFlags::NORMAL);
            // skip any computation if alpha is 0
            if color.alpha() == 0.0 {
                return Ok(());
            }

            let history = &*self.history.borrow();
            let extra_point = *self.extra_point.borrow();
            let radius: f64 = *self.radius.borrow();
            let diamater: f64 = radius * 2.0;

            // Calculate the max value
            let (min, max) = {
                let mut max = *self.max.borrow();
                let min = *self.min.borrow();
                let dynamic = *self.dynamic.borrow();
                if dynamic {
                    // Check for points higher than max
                    for (_, value) in history {
                        if *value > max {
                            max = *value;
                        }
                    }
                    if let Some((_, value)) = extra_point {
                        if value > max {
                            max = value;
                        }
                    }
                }
                (min, max)
            };

            let (margin_top, margin_right, margin_bottom, margin_left) = {
                let margin = styles.margin(gtk::StateFlags::NORMAL);
                (margin.top as f64, margin.right as f64, margin.bottom as f64, margin.left as f64)
            };
            let width = self.obj().allocated_width() as f64 - margin_left - margin_right;
            let height = self.obj().allocated_height() as f64 - margin_top - margin_bottom;

            // Calculate graph points once
            //  Separating this into another function would require pasing a
            //  BarGraphPriv that would hide interior mutability
            let points = {
                let value_range = max - min;
                let time_range = *self.time_range.borrow() as f64;
                let last_updated_at = self.last_updated_at.borrow();
                let points = history
                    .iter()
                    .map(|(instant, value)| {
                        let t = last_updated_at.duration_since(*instant).as_millis() as f64;
                        self.value_to_point(width, height, t / time_range, (value - min) / value_range)
                    })
                    .collect::<VecDeque<(f64, f64)>>();
                points
            };

            let flip_x = *self.flip_x.borrow();
            let flip_y = *self.flip_y.borrow();
            let gradiant_style = &*self.gradiant_style.borrow();
            cr.save()?;
            // Apply color gradiant
            apply_gradiant_style(gradiant_style.as_str(), height, flip_y, &color, cr)?;

            // Draw dots
            for &(x, y) in points.iter() {
                // vertical steps of dots based on configured radius
                let step = diamater + radius;
                //  trim x to an horizontal border + radius to avoid truncated dots
                let x = if flip_x { x - diamater } else { x + diamater };
                if flip_y {
                    // trim y to a vertical border + radius to avoid truncated dots
                    let y = y - diamater;
                    let mut i = height - diamater;
                    while i >= y {
                        cr.arc(x, i, radius, 0.0, 2.0 * std::f64::consts::PI);
                        cr.fill()?;
                        i -= step;
                    }
                } else {
                    // trim y to a vertical border + radius to avoid truncated dots
                    let y = y + diamater;
                    let mut i = diamater;
                    while i <= y {
                        cr.arc(x, i, radius, 0.0, 2.0 * std::f64::consts::PI);
                        cr.fill()?;
                        i += step;
                    }
                }
            }

            cr.restore()?;
            Ok(())
        })();

        if let Err(error) = res {
            error_handling_ctx::print_error(error)
        };

        glib::Propagation::Proceed
    }
}

fn apply_gradiant_style(gradiant_style: &str, height: f64, flip_y: bool, color: &gdk::RGBA, cr: &cairo::Context) -> Result<()> {
    // flip vertically the linear gradiant depending on the BarGraph orientation
    let grad = if flip_y {
        cairo::LinearGradient::new(0.0, 0.0, 0.0, height)
    } else {
        cairo::LinearGradient::new(0.0, height, 0.0, 0.0)
    };
    match gradiant_style {
        "fire" => {
            grad.add_color_stop_rgba(0.0, 1.0, 0.0, 0.0, 1.0);
            grad.add_color_stop_rgba(0.5, 1.0, 0.949, 0.0, 1.0);
            grad.add_color_stop_rgba(1.0, 0.168, 0.839, 0.0, 1.0);
            cr.set_source(grad)?;
        }
        "wiretap" => {
            grad.add_color_stop_rgba(0.0, 0.949, 0.443, 0.129, 1.0);
            grad.add_color_stop_rgba(0.5, 0.913, 0.250, 0.341, 1.0);
            grad.add_color_stop_rgba(1.0, 0.541, 0.137, 0.529, 1.0);
            cr.set_source(grad)?;
        }
        "dracula" => {
            grad.add_color_stop_rgba(0.0, 0.862, 0.141, 0.141, 1.0);
            grad.add_color_stop_rgba(1.0, 0.290, 0.337, 0.615, 1.0);
            cr.set_source(grad)?;
        }
        "none" => {
            // when no gradiant is configured, use CSS color
            cr.set_source_rgba(color.red(), color.green(), color.blue(), color.alpha());
        }
        _ => Err(anyhow!("Error, the value: {} for attribute gradiant-style is not valid", gradiant_style))?,
    };
    Ok(())
}
