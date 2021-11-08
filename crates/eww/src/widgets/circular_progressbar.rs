use std::cell::RefCell;
use anyhow::{Result, anyhow};
use glib::{object_subclass, wrapper};
use gtk::{prelude::*, subclass::prelude::*};

use crate::error_handling_ctx;

wrapper! {
    pub struct CircProg(ObjectSubclass<CircProgPriv>)
    @extends gtk::Bin, gtk::Container, gtk::Widget;
}

pub struct CircProgPriv {
    start_at: RefCell<f32>,
    value: RefCell<f32>,
    thickness: RefCell<f32>,
    clockwise: RefCell<bool>,
    content: RefCell<Option<gtk::Widget>>,
}

// This should match the default values from the ParamSpecs
impl Default for CircProgPriv {
    fn default() -> Self {
        CircProgPriv {
            start_at: RefCell::new(0.0),
            value: RefCell::new(0.0),
            thickness: RefCell::new(1.0),
            clockwise: RefCell::new(true),
            content: RefCell::new(None),
        }
    }
}

impl ObjectImpl for CircProgPriv {
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
                    1f32,
                    glib::ParamFlags::READWRITE,
                ),
                glib::ParamSpec::new_float(
                    "start-at",
                    "Starting at",
                    "Starting at",
                    0f32,
                    100f32,
                    0f32,
                    glib::ParamFlags::READWRITE,
                ),
               glib::ParamSpec::new_boolean(
                    "clockwise",
                    "Clockwise",
                    "Clockwise",
                    true,
                    glib::ParamFlags::READWRITE,
                ),

            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, obj: &Self::Type, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "value" => {
                self.value.replace(value.get().unwrap());
                obj.queue_draw(); // Queue a draw call with the updated value
            }
            "thickness" => {
                self.thickness.replace(value.get().unwrap());
            }
            "start-at" => {
                self.start_at.replace(value.get().unwrap());
            }
            "clockwise" => {
                self.clockwise.replace(value.get().unwrap());
            }
            x => panic!("Tried to set inexistant property of CircProg: {}", x,),
        }
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "value" => self.value.borrow().to_value(),
            "start-at" => self.start_at.borrow().to_value(),
            "thickness" => self.thickness.borrow().to_value(),
            "clockwise" => self.clockwise.borrow().to_value(),
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
        glib::Object::new::<Self>(&[]).expect("Failed to create CircularProgress Widget")
    }
}

impl ContainerImpl for CircProgPriv {
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


impl BinImpl for CircProgPriv {}
impl WidgetImpl for CircProgPriv {
    // We overwrite preferred_* so that overflowing content from the children gets cropped
    // Returning just (0,0) kind of works, however, the widget might get shrunk into nothingness
    // in certain conditions, like, for example, having a large enough margin in its parent
    // container
    fn preferred_width(&self, _widget: &Self::Type) -> (i32, i32) {
        if let Some(child) = &*self.content.borrow() {
            let min = i32::min(child.preferred_width().0, child.preferred_height().0);
            let natural = i32::min(child.preferred_width().1, child.preferred_height().1);
            (min, natural)
        }
        else {
            (0,0)
        }
    }

    fn preferred_width_for_height(&self, _widget: &Self::Type, _height: i32) -> (i32, i32) {
        if let Some(child) = &*self.content.borrow() {
            let min = i32::min(child.preferred_width().0, child.preferred_height().0);
            let natural = i32::min(child.preferred_width().1, child.preferred_height().1);
            (min, natural)
        }
        else {
            (0,0)
        }
    }

    fn preferred_height(&self, _widget: &Self::Type) -> (i32, i32) {
        if let Some(child) = &*self.content.borrow() {
            let min = i32::min(child.preferred_width().0, child.preferred_height().0);
            let natural = i32::min(child.preferred_width().1, child.preferred_height().1);
            (min, natural)
        }
        else {
            (0,0)
        }
    }

    fn preferred_height_for_width(&self, _widget: &Self::Type, _width: i32) -> (i32, i32) {
        if let Some(child) = &*self.content.borrow() {
            let min = i32::min(child.preferred_width().0, child.preferred_height().0);
            let natural = i32::min(child.preferred_width().1, child.preferred_height().1);
            (min, natural)
        }
        else {
            (0,0)
        }
    }

    fn draw(&self, widget: &Self::Type, cr: &cairo::Context) -> Inhibit {
        let styles = widget.style_context();
        let value = *self.value.borrow();
        let start_at = *self.start_at.borrow() as f64;

        let thickness = *self.thickness.borrow() as f64;
        let clockwise = *self.clockwise.borrow() as bool;

        // TODO: This works, but if margin is sufficiently large, it consumes the cairo drawing
        let margin = styles.margin(gtk::StateFlags::NORMAL);
        let total_width = widget.allocated_width() as f64;
        let total_height = widget.allocated_height() as f64;
        let circle_width = total_width - margin.left as f64 - margin.right as f64;
        let circle_height = total_height as f64 - margin.top as f64 - margin.bottom as f64;


        let res: Result<()> = try {
            // Padding is not supported
            let fg_color: gdk::RGBA = styles.color(gtk::StateFlags::NORMAL);
            let bg_color: gdk::RGBA = styles.style_property_for_state("background-color", gtk::StateFlags::NORMAL).get()?;
            let center = (total_width / 2.0, total_height / 2.0);
            let outer_ring =  f64::min(circle_width, circle_height) / 2.0;
            let inner_ring =  (f64::min(circle_width, circle_height) / 2.0) - thickness;
            let (start_angle, end_angle) = if clockwise {
                (0.0, perc_to_rad(value as f64))
            }
            else {
                (perc_to_rad(100.0 - value as f64), 0.0)
            };

            cr.save()?;

            // Centering
            cr.translate(center.0, center.1);
            cr.rotate(perc_to_rad(start_at as f64));
            cr.translate(-center.0, -center.1);

            // Background Ring
            cr.move_to(center.0, center.1);
            cr.arc(center.0, center.1, outer_ring, 0.0, perc_to_rad(100.0));
            cr.set_source_rgba(bg_color.red, bg_color.green, bg_color.blue, bg_color.alpha);
            cr.move_to(center.0, center.1);
            cr.arc(center.0, center.1, inner_ring, 0.0, perc_to_rad(100.0));
            cr.set_fill_rule(cairo::FillRule::EvenOdd); // Substract one circle from the other
            cr.fill()?;

            // Foreground Ring
            cr.move_to(center.0, center.1);
            cr.arc(center.0, center.1, outer_ring, start_angle, end_angle);
            cr.set_source_rgba(fg_color.red, fg_color.green, fg_color.blue, fg_color.alpha);
            cr.move_to(center.0, center.1);
            cr.arc(center.0, center.1, inner_ring, start_angle, end_angle);
            cr.set_fill_rule(cairo::FillRule::EvenOdd); // Substract one circle from the other
            cr.fill()?;
            cr.restore()?;

            // Draw the children widget clipping it to the center
            if let Some(child) = &*self.content.borrow() {
                cr.save()?;

                // Center circular clip
                cr.arc(center.0, center.1, inner_ring+1.0, 0.0, perc_to_rad(100.0));
                cr.set_source_rgba(bg_color.red, 0.0, 0.0, bg_color.alpha);
                cr.clip();

                // Children widget
                widget.propagate_draw(child, &cr);

                cr.reset_clip();
                cr.restore()?;
            }
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
