#[macro_export]
macro_rules! def_widget {
    ($args:ident, $scope_graph:ident, $gtk_widget:ident, {
        $(
            prop( $( $attr_name:ident : $typecast_func:ident $(= $default:expr)?),*) $code:block
        ),+ $(,)?
    }) => {
        $({
            $(
                $args.unhandled_attrs.retain(|a| &a.0 != &::std::stringify!($attr_name).replace('_', "-"));
            )*

            let attr_map: Result<HashMap<eww_shared_util::AttrName, simplexpr::SimplExpr>> = try {
                ::maplit::hashmap! {
                    $(
                        eww_shared_util::AttrName(::std::stringify!($attr_name).to_owned()) =>
                            def_widget!(@get_value $args, &::std::stringify!($attr_name).replace('_', "-"), $(= $default)?)
                    ),*
                }
            };
            if let Ok(attr_map) = attr_map {
                let required_vars = attr_map.values().flat_map(|expr| expr.collect_var_refs()).collect();
                $args.scope_graph.register_listener(
                    $args.calling_scope,
                        crate::state::scope::Listener {
                        needed_variables: required_vars,
                        f: Box::new({
                            let $gtk_widget = $gtk_widget.clone();
                            move |$scope_graph, values| {
                                // value is a map of all the variables that are required to evaluate the
                                // attributes expression.
                                $(
                                    let attr_name: &str = ::std::stringify!($attr_name);
                                    let $attr_name = attr_map.get(attr_name)
                                        .context("Missing attribute, this should never happen")?
                                        .eval(&values)?
                                        .$typecast_func()?;
                                )*
                                $code
                                Ok(())
                            }
                        }),
                    },
                )?;
            }
        })+
    };


    (@get_value $args:ident, $name:expr, = $default:expr) => {
        $args.widget_use.attrs.ast_optional::<simplexpr::SimplExpr>($name)?.clone().unwrap_or(simplexpr::SimplExpr::synth_literal($default))
    };

    (@get_value $args:ident, $name:expr,) => {
        $args.widget_use.attrs.ast_required::<simplexpr::SimplExpr>($name)?.clone()
    }
}

