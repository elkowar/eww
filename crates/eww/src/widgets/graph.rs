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
    join: RefCell<String>,
    range: RefCell<u64>,
    history: RefCell<VecDeque<(std::time::Instant, f64)>>,
}

impl Default for GraphPriv {
    fn default() -> Self {
        Self {
            value: RefCell::new(0.0),
            thickness: RefCell::new(1.0),
            join: RefCell::new("miter".to_string()),
            range: RefCell::new(10),
            history: RefCell::new(VecDeque::new()),
        }
    }
}

fn update_history(graph: &GraphPriv, v: (std::time::Instant, f64)) {
    let mut history = graph.history.borrow_mut();
    let mut last_value = None;
    while let Some(entry) = history.front() {
        if std::time::Instant::now().duration_since(entry.0).as_millis() as u64 > *graph.range.borrow() {
            last_value = history.pop_front();
        } else {
            break;
        }
    }

    if let Some(v) = last_value {
        history.push_front(v);
    }
    history.push_back(v);
}

impl ObjectImpl for GraphPriv {
    fn properties() -> &'static [glib::ParamSpec] {
        use once_cell::sync::Lazy;
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpec::new_double("value", "Value", "The value", 0f64, 100f64, 0f64, glib::ParamFlags::READWRITE),
                glib::ParamSpec::new_double(
                    "thickness",
                    "Thickness",
                    "The Thickness",
                    0f64,
                    100f64,
                    1f64,
                    glib::ParamFlags::READWRITE,
                ),
                glib::ParamSpec::new_uint64("range", "Range", "The Range", 0u64, u64::MAX, 10u64, glib::ParamFlags::READWRITE),
                glib::ParamSpec::new_string("join", "Join", "The Join", Some("miter"), glib::ParamFlags::READWRITE),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, obj: &Self::Type, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "value" => {
                self.value.replace(value.get().unwrap());
                update_history(self, (std::time::Instant::now(), value.get().unwrap()));
                obj.queue_draw();
            }
            "thickness" => {
                self.thickness.replace(value.get().unwrap());
            }
            "range" => {
                self.range.replace(value.get().unwrap());
            }
            "join" => {
                self.join.replace(value.get().unwrap());
            }
            x => panic!("Tried to set inexistant property of Graph: {}", x,),
        }
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "value" => self.value.borrow().to_value(),
            "thickness" => self.thickness.borrow().to_value(),
            "range" => self.range.borrow().to_value(),
            "join" => self.join.borrow().to_value(),
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
            let styles = widget.style_context();
            let thickness = *self.thickness.borrow();
            let join = &*self.join.borrow();
            let history = &*self.history.borrow();
            let range = *self.range.borrow();
            let color: gdk::RGBA = styles.color(gtk::StateFlags::NORMAL);
            let bg_color: gdk::RGBA = styles.style_property_for_state("background-color", gtk::StateFlags::NORMAL).get()?;

            let margin = styles.margin(gtk::StateFlags::NORMAL);
            let width = widget.allocated_width() as f64 - margin.left as f64 - margin.right as f64;
            let height = widget.allocated_height() as f64 - margin.top as f64 - margin.bottom as f64;

            cr.save()?;

            cr.translate(margin.left as f64, margin.top as f64);

            // Prevent the line from overflowing over the margin in the sides
            cr.rectangle(0.0, 0.0, width, height);
            cr.clip();

            // Line join
            match join.as_str() {
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
                _ => Err(anyhow!("Error, the value: {} for atribute join is not valid", join))?,
            };

            // Calculate points once
            let points = history
                .iter()
                .map(|(t, v)| {
                    let t = std::time::Instant::now().duration_since(*t).as_millis();

                    // BUG: range includes a point outside the range
                    let x = width * (1.0 - (t as f64 / range as f64));
                    let y = height * (1.0 - (v / 100.0));
                    (x, y)
                })
                .collect::<Vec<(f64, f64)>>();

            dbg!(&points);
            // Background (separate, to avoid lines in the bottom / sides)
            if bg_color.alpha > 0.0 {
                if let Some(first_point) = points.first() {
                    cr.line_to(first_point.0, height + margin.bottom as f64);
                }
                for (x, y) in points.iter() {
                    cr.line_to(*x, *y);
                }

                cr.line_to(width, height);
                cr.set_source_rgba(bg_color.red, bg_color.green, bg_color.blue, bg_color.alpha);
                cr.fill()?;
            }

            // Line
            if color.alpha > 0.0 && thickness > 0.0 {
                for (x, y) in points.iter() {
                    cr.line_to(*x, *y);
                }
                cr.set_line_width(thickness);
                cr.set_source_rgba(color.red, color.green, color.blue, color.alpha);
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
