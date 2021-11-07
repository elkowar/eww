use std::{cell::RefCell, collections::VecDeque};
// https://www.figuiere.net/technotes/notes/tn002/
// https://github.com/gtk-rs/examples/blob/master/src/bin/listbox_model.rs
use anyhow::{anyhow, Result};
use glib::{object_subclass, wrapper};
use gtk::{prelude::*, subclass::prelude::*};

use crate::error_handling_ctx;

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
    content: RefCell<Option<gtk::Widget>>,
}

impl Default for GraphPriv {
    fn default() -> Self {
        Self {
            value: RefCell::new(0.0),
            thickness: RefCell::new(1.0),
            join: RefCell::new("miter".to_string()),
            range: RefCell::new(10),
            history: RefCell::new(VecDeque::new()),
            content: RefCell::new(None),
        }
    }
}

fn update_history(graph: &GraphPriv, v: (std::time::Instant, f64)) {
    let mut history = graph.history.borrow_mut();
    history.push_back(v);
    while let Some(entry) = history.front() {
        if std::time::Instant::now().duration_since(entry.0).as_millis() as u64 > *graph.range.borrow() {
            history.pop_front();
        } else {
            break;
        }
    }
}

impl ObjectImpl for GraphPriv {
    // glib_object_impl!();
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
}

impl Graph {
    pub fn new() -> Self {
        glib::Object::new::<Self>(&[]).expect("Failed to create Graph Widget")
    }
}

impl ContainerImpl for GraphPriv {
    fn add(&self, container: &Self::Type, widget: &gtk::Widget) {
        self.parent_add(container, widget);
        self.content.replace(Some(widget.clone()));
    }
}

impl BinImpl for GraphPriv {}
impl WidgetImpl for GraphPriv {
    fn draw(&self, widget: &Self::Type, cr: &cairo::Context) -> Inhibit {
        let styles = widget.style_context();
        let width = widget.allocated_width() as f64;
        let height = widget.allocated_height() as f64;
        let thickness = *self.thickness.borrow();
        let join = &*self.join.borrow();
        let history = &*self.history.borrow();
        let range = *self.range.borrow();

        let res: Result<()> = try {
            let color: gdk::RGBA = styles.color(gtk::StateFlags::NORMAL);

            cr.save()?;

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

            // if let Some(v) = history.front() {
            //     let y = height * (1.0 - (v.1 / 100.0));
            //     cr.move_to(width, y);
            // };

            for (t, v) in history.iter() {
                let t = std::time::Instant::now().duration_since(*t).as_millis();
                let x = width * (1.0 - (t as f64 / range as f64));
                let y = height * (1.0 - (v / 100.0));
                cr.line_to(x, y);
            }

            cr.set_line_width(thickness);
            cr.set_source_rgba(color.red, color.green, color.blue, color.alpha);
            cr.stroke()?;

            cr.restore()?;
        };

        if let Err(error) = res {
            error_handling_ctx::print_error(error)
        };

        if let Some(child) = &*self.content.borrow() {
            widget.propagate_draw(child, &cr);
        }
        gtk::Inhibit(false)
    }
}
