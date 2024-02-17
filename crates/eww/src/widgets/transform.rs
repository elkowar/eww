use anyhow::{anyhow, Result};
use glib::{object_subclass, wrapper};
use glib_macros::Properties;
use gtk::{prelude::*, subclass::prelude::*};
use std::{cell::RefCell, str::FromStr};
use yuck::value::NumWithUnit;

use crate::error_handling_ctx;

wrapper! {
    pub struct Transform(ObjectSubclass<TransformPriv>)
    @extends gtk::Bin, gtk::Container, gtk::Widget;
}

#[derive(Properties)]
#[properties(wrapper_type = Transform)]
pub struct TransformPriv {
    #[property(get, set, nick = "Rotate", blurb = "The Rotation", minimum = f64::MIN, maximum = f64::MAX, default = 0f64)]
    rotate: RefCell<f64>,

    #[property(get, set, nick = "Translate x", blurb = "The X Translation", default = None)]
    translate_x: RefCell<Option<String>>,

    #[property(get, set, nick = "Translate y", blurb = "The Y Translation", default = None)]
    translate_y: RefCell<Option<String>>,

    #[property(get, set, nick = "Scale x", blurb = "The amount to scale in x", default = None)]
    scale_x: RefCell<Option<String>>,

    #[property(get, set, nick = "Scale y", blurb = "The amount to scale in y", default = None)]
    scale_y: RefCell<Option<String>>,

    content: RefCell<Option<gtk::Widget>>,
}

// This should match the default values from the ParamSpecs
impl Default for TransformPriv {
    fn default() -> Self {
        TransformPriv {
            rotate: RefCell::new(0.0),
            translate_x: RefCell::new(None),
            translate_y: RefCell::new(None),
            scale_x: RefCell::new(None),
            scale_y: RefCell::new(None),
            content: RefCell::new(None),
        }
    }
}

impl ObjectImpl for TransformPriv {
    fn properties() -> &'static [glib::ParamSpec] {
        Self::derived_properties()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "rotate" => {
                self.rotate.replace(value.get().unwrap());
                self.obj().queue_draw(); // Queue a draw call with the updated value
            }
            "translate-x" => {
                self.translate_x.replace(value.get().unwrap());
                self.obj().queue_draw(); // Queue a draw call with the updated value
            }
            "translate-y" => {
                self.translate_y.replace(value.get().unwrap());
                self.obj().queue_draw(); // Queue a draw call with the updated value
            }
            "scale-x" => {
                self.scale_x.replace(value.get().unwrap());
                self.obj().queue_draw(); // Queue a draw call with the updated value
            }
            "scale-y" => {
                self.scale_y.replace(value.get().unwrap());
                self.obj().queue_draw(); // Queue a draw call with the updated value
            }
            x => panic!("Tried to set inexistant property of Transform: {}", x,),
        }
    }

    fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        self.derived_property(id, pspec)
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

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}

impl Transform {
    pub fn new() -> Self {
        glib::Object::new::<Self>()
    }
}

impl ContainerImpl for TransformPriv {
    fn add(&self, widget: &gtk::Widget) {
        if let Some(content) = &*self.content.borrow() {
            // TODO: Handle this error when populating children widgets instead
            error_handling_ctx::print_error(anyhow!("Error, trying to add multiple children to a circular-progress widget"));
            self.parent_remove(content);
        }
        self.parent_add(widget);
        self.content.replace(Some(widget.clone()));
    }
}

impl BinImpl for TransformPriv {}
impl WidgetImpl for TransformPriv {
    fn draw(&self, cr: &cairo::Context) -> Inhibit {
        let res: Result<()> = (|| {
            let rotate = *self.rotate.borrow();
            let total_width = self.obj().allocated_width() as f64;
            let total_height = self.obj().allocated_height() as f64;

            cr.save()?;

            let translate_x = match &*self.translate_x.borrow() {
                Some(tx) => NumWithUnit::from_str(tx)?.pixels_relative_to(total_width as i32) as f64,
                None => 0.0,
            };

            let translate_y = match &*self.translate_y.borrow() {
                Some(ty) => NumWithUnit::from_str(ty)?.pixels_relative_to(total_height as i32) as f64,
                None => 0.0,
            };

            let scale_x = match &*self.scale_x.borrow() {
                Some(sx) => NumWithUnit::from_str(sx)?.perc_relative_to(total_width as i32) as f64 / 100.0,
                None => 1.0,
            };

            let scale_y = match &*self.scale_y.borrow() {
                Some(sy) => NumWithUnit::from_str(sy)?.perc_relative_to(total_height as i32) as f64 / 100.0,
                None => 1.0,
            };

            cr.scale(scale_x, scale_y);
            cr.rotate(perc_to_rad(rotate));
            cr.translate(translate_x, translate_y);

            // Children widget
            if let Some(child) = &*self.content.borrow() {
                self.obj().propagate_draw(child, cr);
            }

            cr.restore()?;
            Ok(())
        })();

        if let Err(error) = res {
            error_handling_ctx::print_error(error)
        };

        gtk::Inhibit(false)
    }
}

fn perc_to_rad(n: f64) -> f64 {
    (n / 100f64) * 2f64 * std::f64::consts::PI
}
