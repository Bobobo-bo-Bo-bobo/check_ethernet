extern crate getopts;

use getopts::Options;

use std::env;
use std::process;
use std::fs;

const STATE_OK: i32 = 0;
const STATE_WARNING: i32 = 1;
const STATE_CRITICAL: i32 = 2;
const STATE_UNKNOWN: i32 = 3;

struct Configuration {
    interface: String,
    mtu: i32,
    speed: i32,
    duplex: String,
    report_critical: bool,
}

struct InterfaceState {
    present: bool,
    speed: i32,
    mtu: i32,
    operstate: String,
    duplex: String,
}

struct NagiosStatus {
    critical: Vec<String>,
    warning: Vec<String>,
    ok: Vec<String>,
    unknown: Vec<String>,
}

impl NagiosStatus {
    fn new(cfg: &Configuration, ifs: &InterfaceState) -> NagiosStatus {
        let mut critical = Vec::new();
        let mut warning = Vec::new();
        let mut ok = Vec::new();
        let mut unknown = Vec::new();

        if !ifs.present {
            critical.push("Interface is not present".to_string());
            // no need to check futher parameters
            return NagiosStatus{ critical, warning, ok, unknown };
        }

        if ifs.operstate == "down" {
            critical.push("Interface is DOWN".to_string());
            // no need to check futher parameters
            return NagiosStatus{ critical, warning, ok, unknown };
        }

        if ifs.operstate == "up" {
            ok.push("Interface is up".to_string());
        } else {
            // should never happen!
            unknown.push(format!("Interface is {}", ifs.operstate));
            // no need to check futher parameters
            return NagiosStatus{ critical, warning, ok, unknown };
        }

        // check negotiated interface speed and duplex mode
        if cfg.speed > 0 {
            if ifs.speed > cfg.speed {
                warning.push(format!("Negotiated interface speed ({} MBit/s) is greater than requested interface speed ({} MBit/s)", ifs.speed, cfg.speed));
            } else if ifs.speed < cfg.speed {
                if cfg.report_critical {
                    critical.push(format!("Negotiated interface speed ({} MBit/s) is below requested interface speed ({} MBit/s)", ifs.speed, cfg.speed));
                } else {
                    warning.push(format!("Negotiated interface speed ({} MBit/s) is below requested interface speed ({} MBit/s)", ifs.speed, cfg.speed));
                }
            } else {
                ok.push(format!("Negotiated interface speed is {} MBit/s", ifs.speed));
            }

            // check negotiated duplex mode
            if ifs.duplex != "half" && ifs.duplex != "full" {
                unknown.push(format!("Unknown duplex mode {}", ifs.duplex));
            } else if ifs.duplex != cfg.duplex {
                if cfg.report_critical {
                    critical.push(format!("Negotiated duplex mode is {} instead of {}", ifs.duplex, cfg.duplex));
                } else {
                    warning.push(format!("Negotiated duplex mode is {} instead of {}", ifs.duplex, cfg.duplex));
                }
            } else {
                ok.push(format!("Negotiated duplex mode is {}", ifs.duplex));
            }
        }

        // check MTU
        if cfg.mtu > 0 {
            if ifs.mtu != cfg.mtu {
                if cfg.report_critical {
                    critical.push(format!("MTU size of {} does not match requested MTU size of {}", ifs.mtu, cfg.mtu));
                } else {
                    warning.push(format!("MTU size of {} does not match requested MTU size of {}", ifs.mtu, cfg.mtu));
                }
            } else {
                ok.push(format!("MTU size is {}", ifs.mtu));
            }
        }

        NagiosStatus{ critical, warning, ok, unknown }
    }

    fn print(&self) -> i32 {
        if self.unknown.len() > 0 {
            println!("{}", self.unknown.join(", "));
            return STATE_UNKNOWN;
        };

        if self.critical.len() > 0 {
            println!("{}", self.critical.join(", "));
            return STATE_CRITICAL;
        };

        if self.warning.len() > 0 {
            println!("{}", self.warning.join(", "));
            return STATE_WARNING;
        };
        if self.ok.len() > 0 {
            println!("{}", self.ok.join(", "));
            return STATE_OK;
        };
        return STATE_UNKNOWN;
    }
}

impl InterfaceState {
    fn new(cfg: &Configuration) -> Result<InterfaceState, &'static str> {
        let mut mtu: i32 = -1;
        let mut speed: i32 = -1;
        let operstate: String = "unknown".to_string();
        let duplex: String = "unknown".to_string();
        let mut present: bool = false;

        let mut sysfs_path = "/sys/class/net/".to_owned();
        sysfs_path.push_str(cfg.interface.as_str());

        let mut operstate_file = sysfs_path.clone();
        operstate_file.push_str("/operstate");

        let mut duplex_file = sysfs_path.clone();
        duplex_file.push_str("/duplex");

        let mut mtu_file = sysfs_path.clone();
        mtu_file.push_str("/mtu");

        let mut speed_file = sysfs_path.clone();
        speed_file.push_str("/speed");

        let operstate = match fs::read_to_string(operstate_file) {
            Ok(s) => { s.trim().to_string() },
            Err(_) => { return Ok(InterfaceState{ present, speed, mtu, operstate, duplex }) },
        };

        let duplex = match fs::read_to_string(duplex_file) {
            Ok(s) => { s.trim().to_string() },
            Err(_) => { return Ok(InterfaceState{ present, speed, mtu, operstate, duplex }) },
        };

        let raw_mtu = match fs::read_to_string(mtu_file) {
            Ok(s) => { s.trim().to_string() },
            Err(_) => { return Ok(InterfaceState{ present, speed, mtu, operstate, duplex }) },
        };
        mtu = match raw_mtu.trim().parse() {
            Ok(v) => { v },
            Err(_) => { 
                return Err("Can't convert reported MTU to an integer");
            },
        };

        let raw_speed = match fs::read_to_string(speed_file) {
            Ok(s) => { s.trim().to_string() },
            Err(_) => { return Ok(InterfaceState{ present, speed, mtu, operstate, duplex }) },
        };
        speed = match raw_speed.parse() {
            Ok(v) => { v },
            Err(_) => { return Err("Can't convert reported link speed to an integer"); },
        };

        // if we are at this point we are pretty sure the interface exists
        present = true;

        Ok(InterfaceState{ present, speed, mtu, operstate, duplex })
    }
}


fn usage() {
    println!("check_ethernet version 0.1.0\n\
Copyright (C) by Andreas Maus <maus@ypbind.de>\n\
This program comes with ABSOLUTELY NO WARRANTY.\n\
\n\
check_ethernet is distributed under the Terms of the GNU General\n\
Public License Version 3. (http://www.gnu.org/copyleft/gpl.html)\n\
\n\
Usage: check_ethernet -i <if>|--interface=<if> [-m <mtu>|--mtu=<mtu>] [-s <state>|--state=<state>]   [-C|--critical] [-h|--help]\n\
\n\
    -i <if>             Ethernet interface to check.\n\
    --interface=<if>\n\
\n\
    -m <mtu>            Expceted MTU value for interface.\n\
    --mtu=<mtu>\n\
\n\
    -s <state>          Expceted state. <state> is consists of <speed>[:<mode>] where <speed> is the\n\
    --state=<state>     expected negotiated link speed in MBit/s and <mode> is the negotiated link mode.\n\
                        <mode> can be one of \"half\" or \"full\". Default: 1000:full\n\
\n\
    -C                  Report CRITICAL condition if state is below requested speed or duplex (or both) or MTU size\n\
    --critical          does not match. Default: Report WARNING state\n\
\n\
    -h                  This text\n\
    --help\n\
\n");
}

impl Configuration {
    fn new(argv: &[String], opts: &Options) -> Result<Configuration, &'static str> {
        let opt_match = match opts.parse(&argv[1..]) {
            Ok(o) => { o },
            Err(_) => {
                return Err("Failed to parse command line");
            },
        };

        if opt_match.opt_present("h") {
            usage();
            process::exit(STATE_OK);
        }

        let interface = match opt_match.opt_str("i") {
            Some(a) => { a },
            None => { "".to_string() },
        };

        let mtu_ = match opt_match.opt_str("m") {
            Some(a) => { a },
            None => { "-1".to_string() },
        };
        let mtu: i32 = match mtu_.parse() {
            Ok(v) => { v },
            Err(_) => { 
                return Err("Can't convert MTU to an integer");
            },
        };

        let state_ = match opt_match.opt_str("s") {
            Some(a) => { a },
            None => { "1000:full".to_string() },
        };
        let state_vec_: Vec<&str> = state_.split(":").collect();
        let mut speed: i32 = 1000;
        let mut duplex = "full".to_string();

        if state_vec_.len() > 2 || state_vec_.len() == 0 {
            return Err("Invalid link mode");
        }

        if state_vec_.len() == 1 {
            speed = match state_vec_[0].parse() {
                Ok(v) => { v },
                Err(_) => { return Err("Can't convert link speed to an integer"); },
            };
        } else {
            if state_vec_[0] != "" {
                speed = match state_vec_[0].parse() {
                    Ok(v) => { v },
                    Err(_) => { return Err("Can't convert link speed to an integer"); },
                };
            }
            duplex = state_vec_[1].clone().to_string();
        }

        let mut report_critical: bool = false;

        if opt_match.opt_present("C") {
            report_critical = true;
        };

        if interface == "" {
            return Err("Interface to check is mandatory");
        };

        Ok(Configuration{ interface, mtu, speed, duplex, report_critical })
    }
}

fn main() {
    let argv: Vec<String> = env::args().collect();
    let mut options = Options::new();

    options.optflag("h", "help", "Usage information.");
    options.optopt("i", "interface", "Ethernet interface to check.", "");
    options.optopt("m", "mtu", "Expceted MTU value for interface.", "");
    options.optopt("s", "state", "Expceted state.", "");
    options.optflag("C", "critical", "Report CRITICAL condition if state is below requested speed or duplex (or both) or MTU size does not match.");

    let cfg = Configuration::new(&argv, &options).unwrap_or_else(|err| {
        println!("Error: {}", err);
        process::exit(STATE_UNKNOWN);
    });

    let ifstate = match InterfaceState::new(&cfg) {
        Ok(v) => { v },
        Err(s) => {
            println!("{}", s);
            process::exit(STATE_UNKNOWN);
        },
    };

    let nag_status = NagiosStatus::new(&cfg, &ifstate);
    let result = nag_status.print();
    process::exit(result);
}

