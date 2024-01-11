#[cfg(feature = "tpm-simulator-tests")]
pub mod tcp_simulator {
    use client::Tpm;
    use std::io::{Error, ErrorKind, IoSlice, Read, Result, Write};
    use std::net::TcpStream;
    use std::process::{Child, Command};
    use tpm2_rs_base::errors::{TpmError, TpmResult};
    use zerocopy::big_endian::U32;
    use zerocopy::AsBytes;

    const SIMULATOR_IP: &str = "127.0.0.1";
    // TODO: Either pass ports or get simulator to export ports for multithreaded-use.
    const SIMULATOR_TPM_PORT: u16 = 2321;
    const SIMULATOR_PLAT_PORT: u16 = 2322;

    // Launches the TPM simulator at the given path in a subprocess and powers it up.
    pub fn run_tpm_simulator(simulator_bin: &str) -> Result<(TpmSim, TcpTpm)> {
        let sim_lifeline = TpmSim::new(simulator_bin)?;
        let mut attempts = 0;
        while let Err(err) = start_tcp_tpm(SIMULATOR_IP, SIMULATOR_PLAT_PORT) {
            if attempts == 3 {
                return Err(err);
            }
            attempts += 1;
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        Ok((sim_lifeline, TcpTpm::new(SIMULATOR_IP, SIMULATOR_TPM_PORT)?))
    }

    // Holder that manages the lifetime of the simulator subprocess.
    pub struct TpmSim(Child);
    impl TpmSim {
        fn new(simulator_bin: &str) -> Result<TpmSim> {
            Ok(TpmSim(
                Command::new(format!(".{simulator_bin}"))
                    .current_dir("/")
                    .spawn()?,
            ))
        }
    }
    impl Drop for TpmSim {
        fn drop(&mut self) {
            if let Err(x) = self.0.kill() {
                println!("Failed to stop simulator: {x}");
            }
        }
    }

    // Starts up the TPM simulator with a platform server listening at the give IP/port.
    fn start_tcp_tpm(ip: &str, plat_port: u16) -> Result<()> {
        let mut connection = TcpStream::connect(format!("{ip}:{plat_port}"))?;
        TpmCommand::SignalNvOff.issue_to_platform(&mut connection)?;
        TpmCommand::SignalPowerOff.issue_to_platform(&mut connection)?;
        TpmCommand::SignalPowerOn.issue_to_platform(&mut connection)?;
        TpmCommand::SignalNvOn.issue_to_platform(&mut connection)
    }

    #[repr(u32)]
    enum TpmCommand {
        SignalPowerOn = 1,
        SignalPowerOff = 2,
        SendCommand = 8,
        SignalNvOn = 11,
        SignalNvOff = 12,
    }
    impl TpmCommand {
        // Issues a platform TPM command on the given TCP stream.
        pub fn issue_to_platform(self, connection: &mut TcpStream) -> Result<()> {
            connection.write_all(U32::from(self as u32).as_bytes())?;
            let mut rc = U32::ZERO;
            connection.read_exact(rc.as_bytes_mut())?;
            if rc != U32::ZERO {
                Err(Error::new(
                    ErrorKind::Other,
                    format!("Platform command error {}", rc.get()),
                ))
            } else {
                Ok(())
            }
        }
    }

    #[derive(AsBytes)]
    #[repr(C, packed)]
    struct TcpTpmHeader {
        tcp_cmd: U32,
        locality: u8,
        cmd_len: U32,
    }
    // Provides TCP transport for talking to a TPM simulator.
    pub struct TcpTpm {
        tpm_conn: TcpStream,
    }
    impl TcpTpm {
        pub fn new(ip: &str, tpm_port: u16) -> Result<TcpTpm> {
            Ok(TcpTpm {
                tpm_conn: TcpStream::connect(format!("{ip}:{tpm_port}"))?,
            })
        }

        fn read_tpm_u32(&mut self) -> TpmResult<u32> {
            let mut val = U32::ZERO;
            self.tpm_conn
                .read_exact(val.as_bytes_mut())
                .map_err(|_| TpmError::TSS2_BASE_RC_IO_ERROR)?;
            Ok(val.get())
        }
    }

    impl Tpm for TcpTpm {
        fn transact(&mut self, command: &[u8], response: &mut [u8]) -> TpmResult<()> {
            let cmd_size: u32 = command
                .len()
                .try_into()
                .map_err(|_| TpmError::TSS2_BASE_RC_BAD_SIZE)?;
            let tcp_hdr = TcpTpmHeader {
                tcp_cmd: U32::new(TpmCommand::SendCommand as u32),
                locality: 0,
                cmd_len: U32::new(cmd_size),
            };
            let txed = self
                .tpm_conn
                .write_vectored(&[IoSlice::new(tcp_hdr.as_bytes()), IoSlice::new(command)]);
            if txed.unwrap_or(0) != tcp_hdr.as_bytes().len() + command.len() {
                return Err(TpmError::TSS2_BASE_RC_IO_ERROR);
            }

            // Response contains a u32 size, the TPM response, and then an always-zero u32 trailer.
            let resp_size = self.read_tpm_u32()?;
            if resp_size as usize > response.len() {
                return Err(TpmError::TSS2_BASE_RC_INSUFFICIENT_BUFFER);
            }
            self.tpm_conn
                .read_exact(&mut response[..resp_size as usize])
                .map_err(|_| TpmError::TSS2_BASE_RC_IO_ERROR)?;
            if self.read_tpm_u32()? != 0 {
                return Err(TpmError::TSS2_BASE_RC_IO_ERROR);
            }
            Ok(())
        }
    }
}
