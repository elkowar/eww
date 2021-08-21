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
    type ParentType = gtk::Box;

    const ABSTRACT: bool = false;
    const NAME: &'static str = "CircProg";

    glib_object_subclass!();

    fn class_init(klass: &mut Self::Class) {
        klass.install_properties(&PROPERTIES);
        klass.add_signal("added", glib::SignalFlags::RUN_LAST, &[Type::U32], Type::Unit);
    }

    fn new() -> Self {
        Self { value: RefCell::new(0f32), start_angle: RefCell::new(0f32) }
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

impl BoxImpl for CircProgPriv {}
//impl BinImpl for CircProgPriv {}
impl ContainerImpl for CircProgPriv {}
impl WidgetImpl for CircProgPriv {
    // https://sourcegraph.com/github.com/GNOME/fractal/-/blob/fractal-gtk/src/widgets/clip_container.rs?L119 ???
    fn draw(&self, widget: &gtk::Widget, cr: &cairo::Context) -> Inhibit {
        let styles = widget.get_style_context();
        let value = *self.value.borrow();
        cr.save();
        let width = widget.get_allocated_width() as f64;
        let height = widget.get_allocated_height() as f64;

        #[allow(deprecated)]
        let bg_color = styles.get_background_color(gtk::StateFlags::NORMAL);
        cr.set_source_rgba(bg_color.red, bg_color.green, bg_color.blue, bg_color.alpha);

        cr.move_to(width / 2.0, height / 2.0);
        cr.arc(width / 2.0, height / 2.0, height / 4.0, 0.0, perc_to_rad(value as f64));
        cr.fill();
        gtk::Inhibit(false)
    }

    fn get_request_mode(&self, widget: &gtk::Widget) -> gtk::SizeRequestMode {
        self.parent_get_request_mode(widget)
    }

    //fn size_allocate(&self, widget: &gtk::Widget, allocation: &gdk::Rectangle) {
        ////self.parent_size_allocate(widget, allocation);
        ////widget.downcast_ref::<gtk::Box>().unwrap().size_allocate(allocation)
    //}
}

fn perc_to_rad(n: f64) -> f64 {
    (n / 100f64) * 2f64 * std::f64::consts::PI
}
