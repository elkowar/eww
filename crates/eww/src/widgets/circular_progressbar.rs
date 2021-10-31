use std::cell::RefCell;
// https://www.figuiere.net/technotes/notes/tn002/
// https://github.com/gtk-rs/examples/blob/master/src/bin/listbox_model.rs
use glib::{glib_object_impl, glib_object_subclass, glib_wrapper, subclass, translate::*, Type};
use gtk::{prelude::*, subclass::prelude::*};

glib_wrapper! {
    pub struct CircProg(
        Object<subclass::simple::InstanceStruct<CircProgPriv>,
        subclass::simple::ClassStruct<CircProgPriv>,
        CircProgClass>)
        @extends gtk::Box, gtk::Container, gtk::Widget;

    match fn {
        get_type => || CircProgPriv::get_type().to_glib(),
    }
}

pub struct CircProgPriv {
    start_angle: RefCell<f32>,
    value: RefCell<f32>,

    content: RefCell<Option<gtk::Widget>>,
}

static PROPERTIES: [subclass::Property; 2] = [
    subclass::Property("value", |v| {
        glib::ParamSpec::float(v, "Value", "The value", 0f32, 100f32, 0f32, glib::ParamFlags::READWRITE)
    }),
    subclass::Property("start-angle", |v| {
        glib::ParamSpec::float(v, "Starting angle", "Starting angle", 0f32, 100f32, 0f32, glib::ParamFlags::READWRITE)
    }),
];

impl ObjectImpl for CircProgPriv {
    glib_object_impl!();

    fn constructed(&self, obj: &glib::Object) {
        self.parent_constructed(obj);
        dbg!("constructed");
    }

    fn set_property(&self, _obj: &glib::Object, id: usize, value: &glib::Value) {
        let prop = &PROPERTIES[id];
        match prop.0 {
            "value" => {
                self.value.replace(value.get_some().unwrap());
            }
            "start-angle" => {
                self.start_angle.replace(value.get_some().unwrap());
            }
            x => panic!("Tried to set inexistant property of CircProg: {}", x,),
        }
    }

    fn get_property(&self, _obj: &glib::Object, id: usize) -> Result<glib::Value, ()> {
        let prop = &PROPERTIES[id];
        match prop.0 {
            "value" => Ok(self.value.borrow().to_value()),
            "start-angle" => Ok(self.start_angle.borrow().to_value()),
            x => panic!("Tried to access inexistant property of CircProg: {}", x,),
        }
    }
}

impl ObjectSubclass for CircProgPriv {
    type Class = subclass::simple::ClassStruct<Self>;
    type Instance = subclass::simple::InstanceStruct<Self>;
    type ParentType = gtk::Bin;

    const ABSTRACT: bool = false;
    const NAME: &'static str = "CircProg";

    glib_object_subclass!();

    fn class_init(klass: &mut Self::Class) {
        klass.install_properties(&PROPERTIES);
        klass.add_signal("added", glib::SignalFlags::RUN_LAST, &[Type::U32], Type::Unit);
    }

    fn new() -> Self {
        Self { value: RefCell::new(0f32), start_angle: RefCell::new(0f32), content: RefCell::new(None) }
    }
}
impl CircProg {
    pub fn new() -> Self {
        glib::Object::new(Self::static_type(), &[])
            .expect("Failed to create MyAwesome Widget")
            .downcast()
            .expect("Created MyAwesome Widget is of wrong type")
    }
}
impl ContainerImpl for CircProgPriv {
    fn add(&self, container: &gtk::Container, widget: &gtk::Widget) {
        self.parent_add(container, widget);
        self.content.replace(Some(widget.clone()));
    }
}
impl BinImpl for CircProgPriv {}
impl WidgetImpl for CircProgPriv {
    // https://sourcegraph.com/github.com/GNOME/fractal/-/blob/fractal-gtk/src/widgets/clip_container.rs?L119 ???
    // https://stackoverflow.com/questions/50283367/drawingarea-fill-area-outside-a-region
    fn draw(&self, widget: &gtk::Widget, cr: &cairo::Context) -> Inhibit {
        let styles = widget.get_style_context();
        let value = *self.value.borrow();
        let start_angle = *self.start_angle.borrow() as f64;
        let width = widget.get_allocated_width() as f64;
        let height = widget.get_allocated_height() as f64;

        #[allow(deprecated)]
        let bg_color = styles.get_background_color(gtk::StateFlags::NORMAL);

        cr.save();
        cr.translate(width / 2.0, height / 2.0);
        cr.rotate(perc_to_rad(start_angle as f64));
        cr.translate(-width / 2.0, -height / 2.0);
        cr.set_source_rgba(bg_color.red, bg_color.green, bg_color.blue, bg_color.alpha);
        cr.move_to(width / 2.0, height / 2.0);
        cr.arc(width / 2.0, height / 2.0, f64::min(width, height) / 2.0, 0.0, perc_to_rad(value as f64));
        cr.set_source_rgba(20.0, bg_color.green, bg_color.blue, bg_color.alpha);
        cr.move_to(width / 2.0, height / 2.0);
        cr.arc(width / 2.0, height / 2.0, f64::min(width, height) / 3.0, 0.0, perc_to_rad(value as f64));
        cr.set_fill_rule(cairo::FillRule::EvenOdd); // Substract one circle from the other
        cr.fill();
        cr.restore();

        if let Some(child) = &*self.content.borrow() {
            widget.downcast_ref::<gtk::Container>().unwrap().propagate_draw(child, &cr);
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
