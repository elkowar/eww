use std::{cell::RefCell, collections::VecDeque};
// https://www.figuiere.net/technotes/notes/tn002/
// https://github.com/gtk-rs/examples/blob/master/src/bin/listbox_model.rs
use anyhow::{anyhow, Result};
use glib::{object_subclass, wrapper};
use gtk::{prelude::*, subclass::prelude::*};

use crate::error_handling_ctx;

// This widget shouldn't be a Bin/Container but I've not been
//  able to subclass just a gtk::Widget
wrapper! {
    pub struct Graph(ObjectSubclass<GraphPriv>)
    @extends gtk::Bin, gtk::Container, gtk::Widget;
}

pub struct GraphPriv {
    value: RefCell<f64>,
    thickness: RefCell<f64>,
    line_style: RefCell<String>,
    min: RefCell<f64>,
    max: RefCell<f64>,
    dynamic: RefCell<bool>,
    time_range: RefCell<u64>,
    history: RefCell<VecDeque<(std::time::Instant, f64)>>,
    extra_point: RefCell<Option<(std::time::Instant, f64)>>,
    last_updated_at: RefCell<std::time::Instant>,
}

impl Default for GraphPriv {
    fn default() -> Self {
        Self {
            value: RefCell::new(0.0),
            thickness: RefCell::new(1.0),
            line_style: RefCell::new("miter".to_string()),
            min: RefCell::new(0.0),
            max: RefCell::new(100.0),
            dynamic: RefCell::new(true),
            time_range: RefCell::new(10),
            history: RefCell::new(VecDeque::new()),
            extra_point: RefCell::new(None),
            last_updated_at: RefCell::new(std::time::Instant::now()),
        }
    }
}

impl GraphPriv {
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
}

impl ObjectImpl for GraphPriv {
    fn properties() -> &'static [glib::ParamSpec] {
        use once_cell::sync::Lazy;
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpecDouble::new("value", "Value", "The value", 0f64, f64::MAX, 0f64, glib::ParamFlags::READWRITE),
                glib::ParamSpecDouble::new(
                    "thickness",
                    "Thickness",
                    "The Thickness",
                    0f64,
                    100f64,
                    1f64,
                    glib::ParamFlags::READWRITE,
                ),
                glib::ParamSpecDouble::new(
                    "max",
                    "Maximum Value",
                    "The Maximum Value",
                    0f64,
                    f64::MAX,
                    100f64,
                    glib::ParamFlags::READWRITE,
                ),
                glib::ParamSpecDouble::new(
                    "min",
                    "Minumum Value",
                    "The Minimum Value",
                    0f64,
                    f64::MAX,
                    0f64,
                    glib::ParamFlags::READWRITE,
                ),
                glib::ParamSpecBoolean::new("dynamic", "Dynamic", "If it is dynamic", true, glib::ParamFlags::READWRITE),
                glib::ParamSpecUInt64::new(
                    "time-range",
                    "Time Range",
                    "The Time Range",
                    0u64,
                    u64::MAX,
                    10u64,
                    glib::ParamFlags::READWRITE,
                ),
                glib::ParamSpecString::new(
                    "line-style",
                    "Line Style",
                    "The Line Style",
                    Some("miter"),
                    glib::ParamFlags::READWRITE,
                ),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, obj: &Self::Type, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "value" => {
                let value = value.get().unwrap();
                self.value.replace(value);
                self.update_history((std::time::Instant::now(), value));
                obj.queue_draw();
            }
            "thickness" => {
                self.thickness.replace(value.get().unwrap());
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
            "line-style" => {
                self.line_style.replace(value.get().unwrap());
            }
            x => panic!("Tried to set inexistant property of Graph: {}", x,),
        }
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "value" => self.value.borrow().to_value(),
            "thickness" => self.thickness.borrow().to_value(),
            "max" => self.max.borrow().to_value(),
            "min" => self.min.borrow().to_value(),
            "dynamic" => self.dynamic.borrow().to_value(),
            "time-range" => self.time_range.borrow().to_value(),
            "line-style" => self.line_style.borrow().to_value(),
            x => panic!("Tried to access inexistant property of Graph: {}", x,),
        }
    }
}

#[object_subclass]
impl ObjectSubclass for GraphPriv {
    type ParentType = gtk::Bin;
    type Type = Graph;

    const NAME: &'static str = "Graph";

    fn class_init(klass: &mut Self::Class) {
        klass.set_css_name("graph");
    }
}

impl Graph {
    pub fn new() -> Self {
        glib::Object::new::<Self>(&[]).expect("Failed to create Graph Widget")
    }
}

impl ContainerImpl for GraphPriv {
    fn add(&self, _container: &Self::Type, _widget: &gtk::Widget) {
        error_handling_ctx::print_error(anyhow!("Error, Graph widget shoudln't have any children"));
    }
}

impl BinImpl for GraphPriv {}
impl WidgetImpl for GraphPriv {
    fn preferred_width(&self, _widget: &Self::Type) -> (i32, i32) {
        let thickness = *self.thickness.borrow() as i32;
        (thickness, thickness)
    }

    fn preferred_width_for_height(&self, _widget: &Self::Type, height: i32) -> (i32, i32) {
        (height, height)
    }

    fn preferred_height(&self, _widget: &Self::Type) -> (i32, i32) {
        let thickness = *self.thickness.borrow() as i32;
        (thickness, thickness)
    }

    fn preferred_height_for_width(&self, _widget: &Self::Type, width: i32) -> (i32, i32) {
        (width, width)
    }

    fn draw(&self, widget: &Self::Type, cr: &cairo::Context) -> Inhibit {
        let res: Result<()> = try {
            let history = &*self.history.borrow();
            let extra_point = *self.extra_point.borrow();

            // Calculate the max value
            let (min, max) = {
                let mut max = *self.max.borrow();
                let min = *self.min.borrow();
                let dynamic = *self.dynamic.borrow() as bool;
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

            let styles = widget.style_context();
            let (margin_top, margin_right, margin_bottom, margin_left) = {
                let margin = styles.margin(gtk::StateFlags::NORMAL);
                (margin.top as f64, margin.right as f64, margin.bottom as f64, margin.left as f64)
            };
            let width = widget.allocated_width() as f64 - margin_left - margin_right;
            let height = widget.allocated_height() as f64 - margin_top - margin_bottom;

            // Calculate graph points once
            //  Separating this into another function would require pasing a
            //  GraphPriv that would hide interior mutability
            let points = {
                let value_range = max - min;
                let time_range = *self.time_range.borrow() as f64;
                let last_updated_at = self.last_updated_at.borrow();
                let mut points = history
                    .iter()
                    .map(|(instant, value)| {
                        let t = last_updated_at.duration_since(*instant).as_millis() as f64;
                        let x = width * (1.0 - (t / time_range));
                        let y = height * (1.0 - ((value - min) / value_range));
                        (x, y)
                    })
                    .collect::<VecDeque<(f64, f64)>>();

                // Aad an extra point outside of the graph to extend the line to the left
                if let Some((instant, value)) = extra_point {
                    let t = last_updated_at.duration_since(instant).as_millis() as f64;
                    let x = -width * ((t - time_range) / time_range);
                    let y = height * (1.0 - ((value - min) / value_range));
                    points.push_front((x, y));
                }
                points
            };

            // Actually draw the graph
            cr.save()?;
            cr.translate(margin_left, margin_top);
            cr.rectangle(0.0, 0.0, width, height);
            cr.clip();

            // Draw Background
            let bg_color: gdk::RGBA = styles.style_property_for_state("background-color", gtk::StateFlags::NORMAL).get()?;
            if bg_color.alpha() > 0.0 {
                if let Some(first_point) = points.front() {
                    cr.line_to(first_point.0, height + margin_bottom);
                }
                for (x, y) in points.iter() {
                    cr.line_to(*x, *y);
                }
                cr.line_to(width, height);

                cr.set_source_rgba(bg_color.red(), bg_color.green(), bg_color.blue(), bg_color.alpha());
                cr.fill()?;
            }

            // Draw Line
            let line_color: gdk::RGBA = styles.color(gtk::StateFlags::NORMAL);
            let thickness = *self.thickness.borrow();
            if line_color.alpha() > 0.0 && thickness > 0.0 {
                for (x, y) in points.iter() {
                    cr.line_to(*x, *y);
                }

                let line_style = &*self.line_style.borrow();
                apply_line_style(line_style.as_str(), cr)?;
                cr.set_line_width(thickness);
                cr.set_source_rgba(line_color.red(), line_color.green(), line_color.blue(), line_color.alpha());
                cr.stroke()?;
            }

            cr.reset_clip();
            cr.restore()?;
        };

        if let Err(error) = res {
            error_handling_ctx::print_error(error)
        };

        gtk::Inhibit(false)
    }
}

fn apply_line_style(style: &str, cr: &cairo::Context) -> Result<()> {
    match style {
        "miter" => {
            cr.set_line_cap(cairo::LineCap::Butt);
            cr.set_line_join(cairo::LineJoin::Miter);
        }
        "bevel" => {
            cr.set_line_cap(cairo::LineCap::Square);
            cr.set_line_join(cairo::LineJoin::Bevel);
        }
        "round" => {
            cr.set_line_cap(cairo::LineCap::Round);
            cr.set_line_join(cairo::LineJoin::Round);
        }
        _ => Err(anyhow!("Error, the value: {} for atribute join is not valid", style))?,
    };
    Ok(())
}
