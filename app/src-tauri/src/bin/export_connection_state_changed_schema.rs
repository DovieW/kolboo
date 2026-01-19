fn main() {
    kolboo_lib::schema_export::print_schema::<kolboo_lib::ConnectionStateChangedPayload>(
        "connection-state-changed",
    );
}
