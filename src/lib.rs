#[macro_use]
extern crate lazy_static;

pub mod pm1;

// use pm1::PM1;

// pub struct ChassisThreads(Vec<std::thread::JoinHandle<PM1>>);

// #[macro_export]
// macro_rules! rtk_threads {
//     ($block:expr) => {
//         RTKThreads::open_all($block)
//     };
//     ($($x:expr)+; $block:expr ) => {
//         RTKThreads::open_some(&[$(String::from($x),)*], $block)
//     };
// }

// impl ChassisThreads {
//     /// 打开一些串口
//     pub fn open_some<F>(paths: &[String], block: F) -> Self
//     where
//         F: 'static + Send + Clone + FnOnce(String, &mut PM1),
//     {
//         Self(
//             paths
//                 .iter()
//                 .filter_map(may_open)
//                 .map(|(name, port)| {
//                     let f = block.clone();
//                     std::thread::spawn(move || {
//                         let mut chassis = PM1::new(port);
//                         f(name, &mut chassis);
//                         chassis
//                     })
//                 })
//                 .collect::<Vec<_>>(),
//         )
//     }

//     /// 打开所有串口
//     pub fn open_all<F>(block: F) -> Self
//     where
//         F: 'static + Send + Clone + FnOnce(String, &mut Chassis),
//     {
//         Self::open_some(Port::list().as_slice(), block)
//     }

//     /// 阻塞
//     pub fn join(self) {
//         for thread in self.0 {
//             thread.join().unwrap();
//         }
//     }
// }

// fn may_open(name: &String) -> Option<(String, Port)> {
//     let path: String = if cfg!(target_os = "windows") {
//         if let Some((i, _)) = name.rmatch_indices("COM").next() {
//             name.as_str()[i..name.len() - 1].into()
//         } else {
//             return None;
//         }
//     } else {
//         name.clone()
//     };

//     match Port::open(path.as_str(), 230400) {
//         Ok(port) => {
//             println!("reading from {}", path);
//             Some((path, port))
//         }
//         Err(e) => {
//             eprintln!("failed to open {}: {}", path, e);
//             None
//         }
//     }
// }
