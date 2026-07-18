use linkme::distributed_slice;

#[distributed_slice]
pub static TUNE_PARAMS: [TuneParam];

pub struct TuneParam {
	pub name: &'static str,
	pub value: *mut f64,
	pub default_value: f64,
	pub min: f64,
	pub max: f64,
}

unsafe impl Sync for TuneParam {}

impl TuneParam {
	#[inline(always)]
	pub fn set(&self, value: f64) {
		unsafe {
			*self.value = value;
		}
	}

	#[inline(always)]
	pub fn get(&self) -> f64 {
		unsafe { *self.value }
	}
}

#[macro_export]
macro_rules! tune {
	($name:ident, $default_value:expr, $min:expr, $max:expr) => {
		#[allow(nonstandard_style)]
		mod $name {
			#[allow(nonstandard_style)]
			pub static mut $name: f64 = $default_value;
		}

		#[allow(nonstandard_style)]
		#[linkme::distributed_slice($crate::tuning::TUNE_PARAMS)]
		static $name: $crate::tuning::TuneParam = $crate::tuning::TuneParam {
			name: stringify!($name),
			value: &raw mut $name::$name,
			default_value: $default_value,
			min: $min,
			max: $max,
		};
	};
}

pub fn print_tune_info() {
	for param in TUNE_PARAMS {
		print!(
			"{},{},{},{},",
			param.name, param.default_value, param.min, param.max
		);
	}

	if !TUNE_PARAMS.is_empty() {
		println!();
	}
}
