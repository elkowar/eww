use anyhow::{anyhow, Result};
use glib::{object_subclass, wrapper};
use gtk::{prelude::*, subclass::prelude::*};
use std::cell::RefCell;

use crate::error_handling_ctx;

wrapper! {
    pub struct Transform(ObjectSubclass<TransformPriv>)
    @extends gtk::Bin, gtk::Container, gtk::Widget;
}

pub struct TransformPriv {
    translate_x: RefCell<f64>,
    translate_y: RefCell<f64>,
    rotate: RefCell<f64>,
    scale_x: RefCell<f64>,
    scale_y: RefCell<f64>,
    content: RefCell<Option<gtk::Widget>>,
}

// This should match the default values from the ParamSpecs
impl Default for TransformPriv {
    fn default() -> Self {
        TransformPriv {
            translate_x: RefCell::new(0.0),
            translate_y: RefCell::new(0.0),
            rotate: RefCell::new(0.0),
            scale_x: RefCell::new(0.0),
            scale_y: RefCell::new(0.0),
            content: RefCell::new(None),
        }
    }
}

impl ObjectImpl for TransformPriv {
    fn properties() -> &'static [glib::ParamSpec] {
        use once_cell::sync::Lazy;
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpec::new_double("rotate", "Rotate", "The Rotation", f64::MIN, f64::MAX, 0f64, glib::ParamFlags::READWRITE),
                glib::ParamSpec::new_double("translate-x", "Translate x", "The Translation x", f64::MIN, f64::MAX, 0f64, glib::ParamFlags::READWRITE),
                glib::ParamSpec::new_double("translate-y", "Translate y", "The Translation y", f64::MIN, f64::MAX, 0f64, glib::ParamFlags::READWRITE),
                glib::ParamSpec::new_double("scale-x", "Scale x", "The amount to scale in x", f64::MIN, f64::MAX, 0f64, glib::ParamFlags::READWRITE),
                glib::ParamSpec::new_double("scale-y", "Scale y", "The amount to scale in y", f64::MIN, f64::MAX, 0f64, glib::ParamFlags::READWRITE),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, obj: &Self::Type, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "rotate" => {
                self.rotate.replace(value.get().unwrap());
                obj.queue_draw(); // Queue a draw call with the updated value
            }
            "translate-x" => {
                self.translate_x.replace(value.get().unwrap());
                obj.queue_draw(); // Queue a draw call with the updated value
            }
            "translate-y" => {
                self.translate_y.replace(value.get().unwrap());
                obj.queue_draw(); // Queue a draw call with the updated value
            }
            "scale-x" => {
                self.scale_x.replace(value.get().unwrap());
                obj.queue_draw(); // Queue a draw call with the updated value
            }
            "scale-y" => {
                self.scale_y.replace(value.get().unwrap());
                obj.queue_draw(); // Queue a draw call with the updated value
            }
            x => panic!("Tried to set inexistant property of Transform: {}", x,),
        }
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "rotate" => self.rotate.borrow().to_value(),
            "translate_x" => self.translate_x.borrow().to_value(),
            "translate_y" => self.translate_y.borrow().to_value(),
            "scale_x" => self.scale_x.borrow().to_value(),
            "scale_y" => self.scale_y.borrow().to_value(),
            x => panic!("Tried to access inexistant property of Transform: {}", x,),
        }
    }
}

#[object_subclass]
impl ObjectSubclass for TransformPriv {
    type ParentType = gtk::Bin;
    type Type = Transform;

    const NAME: &'static str = "Transform";

    fn class_init(klass: &mut Self::Class) {
        klass.set_css_name("transform");
    }
}

impl Transform {
    pub fn new() -> Self {
        glib::Object::new::<Self>(&[]).expect("Failed to create Transform Widget")
    }
}

impl ContainerImpl for TransformPriv {
    fn add(&self, container: &Self::Type, widget: &gtk::Widget) {
        if let Some(content) = &*self.content.borrow() {
            // TODO: Handle this error when populating children widgets instead
            error_handling_ctx::print_error(anyhow!("Error, trying to add multiple children to a circular-progress widget"));
            self.parent_remove(container, content);
        }
        self.parent_add(container, widget);
        self.content.replace(Some(widget.clone()));
    }
}

impl BinImpl for TransformPriv {}
impl WidgetImpl for TransformPriv {
    fn draw(&self, widget: &Self::Type, cr: &cairo::Context) -> Inhibit {
        let res: Result<()> = try {
            let translate_x = *self.translate_x.borrow();
            let translate_y = *self.translate_y.borrow();
            let rotate = *self.rotate.borrow();
            let scale_x = *self.scale_x.borrow();
            let scale_y = *self.scale_y.borrow();

            cr.save()?;

            // Do not change the order
            if rotate != 0.0 {
                cr.rotate(perc_to_rad(rotate));
            }

            if translate_x != 0.0 || translate_y != 0.0 {
                cr.translate(translate_x, translate_y);
            }

            if scale_x != 0.0 || scale_y != 0.0 {
                cr.scale(scale_x, scale_y);
            }

            // Children widget
            if let Some(child) = &*self.content.borrow() {
                widget.propagate_draw(child, &cr);
            }

            cr.restore()?;
        };

        if let Err(error) = res {
            error_handling_ctx::print_error(error)
        };

        gtk::Inhibit(false)
    }
}

fn perc_to_rad(n: f64) -> f64 {
    (n / 100f64) * 2f64 * std::f64::consts::PI
}
