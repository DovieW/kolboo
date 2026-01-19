fn main() {
    kolboo_lib::schema_export::print_schema::<kolboo_lib::EmptyEventPayload>(
        "overlay-hide-requested",
    );
}
