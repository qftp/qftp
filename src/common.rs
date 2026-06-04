const QFTP_PROTOCOL: &[u8] = b"qftp";
const MAX_CONNECTIONS: u64 = 10;
const STREAM_BUFFER_BYTES: u64 = 1024 * 1024; // 1 MB
pub const QFTP_PORT: u16 = 8080;

pub fn new_quiche_config(is_server: bool) -> Result<quiche::Config, quiche::Error> {
    let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION)?;

    // Required settings
    config.set_application_protos(&[QFTP_PROTOCOL])?;
    config.set_initial_max_streams_bidi(MAX_CONNECTIONS);
    config.set_initial_max_streams_uni(MAX_CONNECTIONS);
    config.set_initial_max_data(STREAM_BUFFER_BYTES);
    config.set_initial_max_stream_data_bidi_local(STREAM_BUFFER_BYTES);
    config.set_initial_max_stream_data_bidi_remote(STREAM_BUFFER_BYTES);
    config.set_initial_max_stream_data_uni(STREAM_BUFFER_BYTES);

    // Congestion control
    config.set_cc_algorithm(quiche::CongestionControlAlgorithm::CUBIC);

    // TODO: TLS
    if is_server {
        config.load_cert_chain_from_pem_file("cert.crt")?;
        config.load_priv_key_from_pem_file("cert.key")?;
    } else {
        config.verify_peer(false);
    }

    return Ok(config);
}
