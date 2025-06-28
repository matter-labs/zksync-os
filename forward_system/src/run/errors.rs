use basic_bootloader::bootloader::errors::BootloaderSubsystemError;

zk_ee::define_subsystem!(Forward,
                  cascade WrappedError {
                      Bootloader(BootloaderSubsystemError),
                  }
);
