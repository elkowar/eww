use anyhow::{anyhow, Result};
use gtk::glib::{self, object_subclass, prelude::*, wrapper, Properties};
use gtk::{cairo, gdk, prelude::*, subclass::prelude::*};
use std::cell::RefCell;

use crate::error_handling_ctx;

wrapper! {
    pub struct CircProg(ObjectSubclass<CircProgPriv>)
    @extends gtk::Bin, gtk::Container, gtk::Widget;
}

#[derive(Properties)]
#[properties(wrapper_type = CircProg)]
pub struct CircProgPriv {
    #[property(get, set, nick = "Starting at", blurb = "Starting at", minimum = 0f64, maximum = 100f64, default = 0f64)]
    start_at: RefCell<f64>,

    #[property(get, set, nick = "Value", blurb = "The value", minimum = 0f64, maximum = 100f64, default = 0f64)]
    value: RefCell<f64>,

    #[property(get, set, nick = "Thickness", blurb = "Thickness", minimum = 0f64, maximum = 100f64, default = 1f64)]
    thickness: RefCell<f64>,

    #[property(get, set, nick = "Clockwise", blurb = "Clockwise", default = true)]
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
        Self::derived_properties()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "value" => {
                self.value.replace(value.get().unwrap());
                self.obj().queue_draw(); // Queue a draw call with the updated value
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

    fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        self.derived_property(id, pspec)
    }
}

#[object_subclass]
impl ObjectSubclass for CircProgPriv {
    type ParentType = gtk::Bin;
    type Type = CircProg;

    const NAME: &'static str = "CircProg";

    fn class_init(klass: &mut Self::Class) {
        klass.set_css_name("circular-progress");
    }
}

impl Default for CircProg {
    fn default() -> Self {
        Self::new()
    }
}

impl CircProg {
    pub fn new() -> Self {
        glib::Object::new::<Self>()
    }
}

impl ContainerImpl for CircProgPriv {
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

fn calc_widget_lowest_preferred_dimension(widget: &gtk::Widget) -> (i32, i32) {
    let preferred_width = widget.preferred_width();
    let preferred_height = widget.preferred_height();
    let min_lowest = i32::min(preferred_width.0, preferred_height.0);
    let natural_lowest = i32::min(preferred_width.1, preferred_height.1);
    (min_lowest, natural_lowest)
}

impl BinImpl for CircProgPriv {}

impl WidgetImpl for CircProgPriv {
    // We overwrite preferred_* so that overflowing content from the children gets cropped
    //  We return min(child_width, child_height)
    fn preferred_width(&self) -> (i32, i32) {
        let styles = self.obj().style_context();
        let margin = styles.margin(gtk::StateFlags::NORMAL);

        if let Some(child) = &*self.content.borrow() {
            let (min_child, natural_child) = calc_widget_lowest_preferred_dimension(child);
            (min_child + margin.right as i32 + margin.left as i32, natural_child + margin.right as i32 + margin.left as i32)
        } else {
            let empty_width = (2 * *self.thickness.borrow() as i32) + margin.right as i32 + margin.left as i32;
            (empty_width, empty_width)
        }
    }

    fn preferred_width_for_height(&self, _height: i32) -> (i32, i32) {
        self.preferred_width()
    }

    fn preferred_height(&self) -> (i32, i32) {
        let styles = self.obj().style_context();
        let margin = styles.margin(gtk::StateFlags::NORMAL);

        if let Some(child) = &*self.content.borrow() {
            let (min_child, natural_child) = calc_widget_lowest_preferred_dimension(child);
            (min_child + margin.bottom as i32 + margin.top as i32, natural_child + margin.bottom as i32 + margin.top as i32)
        } else {
            let empty_height = (2 * *self.thickness.borrow() as i32) + margin.right as i32 + margin.left as i32;
            (empty_height, empty_height)
        }
    }

    fn preferred_height_for_width(&self, _width: i32) -> (i32, i32) {
        self.preferred_height()
    }

    fn draw(&self, cr: &cairo::Context) -> glib::Propagation {
        let res: Result<()> = (|| {
            let value = *self.value.borrow();
            let start_at = *self.start_at.borrow();
            let thickness = *self.thickness.borrow();
            let clockwise = *self.clockwise.borrow();

            let styles = self.obj().style_context();
            let margin = styles.margin(gtk::StateFlags::NORMAL);
            // Padding is not supported yet
            let fg_color: gdk::RGBA = styles.color(gtk::StateFlags::NORMAL);
            let bg_color: gdk::RGBA = styles.style_property_for_state("background-color", gtk::StateFlags::NORMAL).get()?;
            let (start_angle, end_angle) =
                if clockwise { (0.0, perc_to_rad(value)) } else { (perc_to_rad(100.0 - value), 2f64 * std::f64::consts::PI) };

            let total_width = self.obj().allocated_width() as f64;
            let total_height = self.obj().allocated_height() as f64;
            let center = (total_width / 2.0, total_height / 2.0);

            let circle_width = total_width - margin.left as f64 - margin.right as f64;
            let circle_height = total_height - margin.top as f64 - margin.bottom as f64;
            let outer_ring = f64::min(circle_width, circle_height) / 2.0;
            let inner_ring = (f64::min(circle_width, circle_height) / 2.0) - thickness;

            cr.save()?;

            // Centering
            cr.translate(center.0, center.1);
            cr.rotate(perc_to_rad(start_at));
            cr.translate(-center.0, -center.1);

            // Background Ring
            cr.move_to(center.0, center.1);
            cr.arc(center.0, center.1, outer_ring, 0.0, perc_to_rad(100.0));
            cr.set_source_rgba(bg_color.red(), bg_color.green(), bg_color.blue(), bg_color.alpha());
            cr.move_to(center.0, center.1);
            cr.arc(center.0, center.1, inner_ring, 0.0, perc_to_rad(100.0));
            cr.set_fill_rule(cairo::FillRule::EvenOdd); // Substract one circle from the other
            cr.fill()?;

            // Foreground Ring
            cr.move_to(center.0, center.1);
            cr.arc(center.0, center.1, outer_ring, start_angle, end_angle);
            cr.set_source_rgba(fg_color.red(), fg_color.green(), fg_color.blue(), fg_color.alpha());
            cr.move_to(center.0, center.1);
            cr.arc(center.0, center.1, inner_ring, start_angle, end_angle);
            cr.set_fill_rule(cairo::FillRule::EvenOdd); // Substract one circle from the other
            cr.fill()?;
            cr.restore()?;

            // Draw the children widget, clipping it to the inside
            if let Some(child) = &*self.content.borrow() {
                cr.save()?;

                // Center circular clip
                cr.arc(center.0, center.1, inner_ring + 1.0, 0.0, perc_to_rad(100.0));
                cr.set_source_rgba(bg_color.red(), 0.0, 0.0, bg_color.alpha());
                cr.clip();

                // Children widget
                self.obj().propagate_draw(child, cr);

                cr.reset_clip();
                cr.restore()?;
            }
            Ok(())
        })();

        if let Err(error) = res {
            error_handling_ctx::print_error(error)
        };

        glib::Propagation::Proceed
    }
}

fn perc_to_rad(n: f64) -> f64 {
    (n / 100f64) * 2f64 * std::f64::consts::PI
}
