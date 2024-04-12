pub fn load_args<C: clap::Parser>() -> C {
    let args = argfile::expand_args_from(
        std::env::args_os(),
        argfile::parse_fromfile,
        argfile::PREFIX,
    )
    .unwrap();
    C::parse_from(args)
}
