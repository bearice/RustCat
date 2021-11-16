// #![windows_subsystem = "windows"]

use core::mem::MaybeUninit;
use std::{
    sync::{atomic::AtomicBool, Arc},
    thread::sleep,
    time::Duration,
};
use trayicon::*;
use winapi::um::winuser::{self};

mod cpu_usage;
#[allow(dead_code)]
mod icons {
    include!(concat!(env!("OUT_DIR"), "/icons.rs"));
    use trayicon::*;
    pub fn load_icons() -> Vec<Icon> {
        DARK_CAT
            .iter()
            .map(|i| Icon::from_buffer(*i, None, None).unwrap())
            .collect()
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Events {
    Exit,
}

fn main() {
    let (s, r) = std::sync::mpsc::channel::<Events>();

    let icons = icons::load_icons();

    let mut tray_icon = TrayIconBuilder::new()
        .sender(s)
        .icon(icons[0].clone())
        .tooltip("Nyan~")
        .menu(MenuBuilder::new().item("E&xit", Events::Exit))
        .build()
        .unwrap();

    let exit = Arc::new(AtomicBool::new(false));
    {
        let exit = exit.clone();
        std::thread::spawn(move || {
            r.iter().for_each(|m| match m {
                Events::Exit => {
                    println!("exit");
                    exit.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            })
        });
    }

    {
        let exit = exit.clone();
        std::thread::spawn(move || {
            let sleep_interval = 10;
            let mut t1 = cpu_usage::get_cpu_totals().unwrap();
            let mut update_counter = 0;
            let mut animate_counter = 0;
            let mut icon_index = 0;
            let mut speed = 200;
            while !exit.load(std::sync::atomic::Ordering::Relaxed) {
                sleep(Duration::from_millis(sleep_interval));
                if animate_counter >= speed {
                    animate_counter = 0;
                    icon_index += 1;
                    icon_index %= icons.len();
                    tray_icon.set_icon(&icons[icon_index]).expect("set_icon");
                }
                animate_counter += sleep_interval;
                if update_counter == 1000 {
                    update_counter = 0;
                    let t2 = cpu_usage::get_cpu_totals().unwrap();
                    let usage = 100.0 - (t2.1 - t1.1) / (t2.0 - t1.0) * 100.0;
                    t1 = t2;
                    speed = (200.0 / f64::max(1.0, f64::min(20.0, usage / 5.0))).round() as u64;
                    println!("CPU Usage: {:.2}% speed: {}", usage, speed);
                    tray_icon
                        .set_tooltip(&format!("CPU Usage: {:.2}%", usage))
                        .expect("set_tooltip");
                }
                update_counter += sleep_interval;
            }
        });
    }
    // Your applications message loop. Because all applications require an
    // application loop, you are best served using an `winit` crate.
    while !exit.load(std::sync::atomic::Ordering::Relaxed) {
        unsafe {
            let mut msg = MaybeUninit::uninit();
            let bret = winuser::GetMessageA(msg.as_mut_ptr(), 0 as _, 0, 0);
            if bret > 0 {
                winuser::TranslateMessage(msg.as_ptr());
                winuser::DispatchMessageA(msg.as_ptr());
            } else {
                break;
            }
        }
    }
}
