use std::cell::RefCell;
// https://www.figuiere.net/technotes/notes/tn002/
// https://github.com/gtk-rs/examples/blob/master/src/bin/listbox_model.rs
use anyhow::Result;
use glib::{object_subclass, wrapper};
use gtk::{prelude::*, subclass::prelude::*};

use crate::error_handling_ctx;

wrapper! {
    pub struct CircProg(ObjectSubclass<CircProgPriv>)
    @extends gtk::Bin, gtk::Container, gtk::Widget;
}

// wrapper! { pub struct CircProg(ObjectSubclass<CircProgPriv>) @extends gtk::Box, gtk::Container, gtk::Widget; }

#[derive(Default)]
pub struct CircProgPriv {
    start_angle: RefCell<f32>,
    value: RefCell<f32>,
    thickness: RefCell<f32>,

    content: RefCell<Option<gtk::Widget>>,
}

impl ObjectImpl for CircProgPriv {
    // glib_object_impl!();
    fn properties() -> &'static [glib::ParamSpec] {
        use once_cell::sync::Lazy;
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpec::new_float("value", "Value", "The value", 0f32, 100f32, 0f32, glib::ParamFlags::READWRITE),
                glib::ParamSpec::new_float(
                    "thickness",
                    "Thickness",
                    "Thickness",
                    0f32,
                    100f32,
                    0f32,
                    glib::ParamFlags::READWRITE,
                ),
                glib::ParamSpec::new_float(
                    "start-angle",
                    "Starting angle",
                    "Starting angle",
                    0f32,
                    100f32,
                    0f32,
                    glib::ParamFlags::READWRITE,
                ),
            ]
        });

        PROPERTIES.as_ref()
    }

    //    fn constructed(&self, obj: &Self::Type) {
    //        self.parent_constructed(obj);
    //    }

    fn set_property(&self, _obj: &Self::Type, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "value" => {
                self.value.replace(value.get().unwrap());
            }
            "thickness" => {
                self.thickness.replace(value.get().unwrap());
            }
            "start-angle" => {
                self.start_angle.replace(value.get().unwrap());
            }
            x => panic!("Tried to set inexistant property of CircProg: {}", x,),
        }
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "value" => self.value.borrow().to_value(),
            "start-angle" => self.start_angle.borrow().to_value(),
            "thickness" => self.thickness.borrow().to_value(),
            x => panic!("Tried to access inexistant property of CircProg: {}", x,),
        }
    }
}

#[object_subclass]
impl ObjectSubclass for CircProgPriv {
    type ParentType = gtk::Bin;
    type Type = CircProg;

    const NAME: &'static str = "CircProg";
}

impl CircProg {
    pub fn new() -> Self {
        glib::Object::new::<Self>(&[]).expect("Failed to create MyAwesome Widget")
    }
}
impl ContainerImpl for CircProgPriv {
    fn add(&self, container: &Self::Type, widget: &gtk::Widget) {
        self.parent_add(container, widget);
        self.content.replace(Some(widget.clone()));
    }
}
impl BinImpl for CircProgPriv {}
impl WidgetImpl for CircProgPriv {
    // https://sourcegraph.com/github.com/GNOME/fractal/-/blob/fractal-gtk/src/widgets/clip_container.rs?L119 ???
    // https://stackoverflow.com/questions/50283367/drawingarea-fill-area-outside-a-region
    fn draw(&self, widget: &Self::Type, cr: &cairo::Context) -> Inhibit {
        let styles = widget.style_context();
        let value = *self.value.borrow();
        let start_angle = *self.start_angle.borrow() as f64;
        let thickness = *self.thickness.borrow() as f64;
        let width = widget.allocated_width() as f64;
        let height = widget.allocated_height() as f64;

        let res: Result<()> = try {
            let bg_color: gdk::RGBA = styles.style_property_for_state("background-color", gtk::StateFlags::NORMAL).get()?;

            cr.save()?;
            cr.translate(width / 2.0, height / 2.0);
            cr.rotate(perc_to_rad(start_angle as f64));
            cr.translate(-width / 2.0, -height / 2.0);
            cr.set_source_rgba(bg_color.red, bg_color.green, bg_color.blue, bg_color.alpha);
            cr.move_to(width / 2.0, height / 2.0);
            cr.arc(width / 2.0, height / 2.0, f64::min(width, height) / 2.0, 0.0, perc_to_rad(value as f64));
            cr.set_source_rgba(bg_color.red, bg_color.green, bg_color.blue, bg_color.alpha);
            cr.move_to(width / 2.0, height / 2.0);
            cr.arc(width / 2.0, height / 2.0, (f64::min(width, height) - thickness) / 2.0, 0.0, perc_to_rad(value as f64));
            cr.set_fill_rule(cairo::FillRule::EvenOdd); // Substract one circle from the other
            cr.fill()?;
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

    // fn get_request_mode(&self, widget: &gtk::Widget) -> gtk::SizeRequestMode {
    //    self.parent_get_request_mode(widget)
    //}

    // fn size_allocate(&self, widget: &gtk::Widget, allocation: &gdk::Rectangle) {
    //    self.parent_size_allocate(widget, allocation);
    //    widget.downcast_ref::<gtk::Bin>().unwrap().size_allocate(allocation)
    //}
}

fn perc_to_rad(n: f64) -> f64 {
    (n / 100f64) * 2f64 * std::f64::consts::PI
}
