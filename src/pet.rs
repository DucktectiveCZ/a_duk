use std::{
    collections::HashMap,
    ffi::OsString,
    fs,
    io,
    fmt::Display,
    path::{
        Path,
        PathBuf
    }
};
use serde::{Deserialize, Serialize};
use mlua::{Function, Lua};

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    TomlDeserializer(toml::de::Error),
    Utf8(OsString),
    InvalidFileName,
    Lua(mlua::Error),
    InvalidObject(&'static str),
}

impl Display for Error {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::IO(e) => format!("IO Error: {e}"),
            Self::TomlDeserializer(e) => format!("Toml deserialization error: {e}"),
            Self::Utf8(s) => format!("Utf8 conversion error: Not a valid UTF8 string: {}", s.to_string_lossy()),
            Self::InvalidFileName => "Invalid file name".to_string(),
            Self::Lua(e) => format!("Lua error: {e}"),
            Self::InvalidObject(msg) => format!("Invalid object: {msg}"),
        })
    }
}

#[derive(Deserialize, Debug)]
pub struct AnimationMetadata {
    pub delay: u64,
}

impl AnimationMetadata {
    pub fn load(path: &Path) -> Result<AnimationMetadata, Error> {
        let toml_string = match fs::read_to_string(path) {
            Ok(val) => val,
            Err(e) => return Err(Error::IO(e)),
        };

        match toml::de::from_str::<Self>(toml_string.as_str()) {
            Ok(val) => Ok(val),
            Err(e) => Err(Error::TomlDeserializer(e)),
        }
    }
}

#[derive(Debug)]
pub struct Animation {
    pub name: String,
    pub metadata: AnimationMetadata,
    pub frames: Vec<String>,
}

impl Animation {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let name = path.file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(||
                Error::IO(io::Error::new(io::ErrorKind::InvalidInput,
                    "Invalid filename")))?
            .to_string();


        let metadata = AnimationMetadata::load(path.join("meta.toml").as_path())?;

        let mut frame_files: Vec<_> = fs::read_dir(path)
            .map_err(Error::IO)?
            .filter_map(Result::ok)
            .filter(|entry| {
                let filename = entry.file_name().to_string_lossy().into_owned();

                filename.ends_with(".txt") &&
                    filename.strip_suffix(".txt")
                        .and_then(|prefix| prefix.parse::<usize>().ok() )
                        .is_some()
            })
            .collect();

        frame_files.sort_by_key(|e| e.file_name()
            .to_string_lossy()
            .strip_suffix(".txt")
            .unwrap()
            .parse::<u32>()
            .unwrap()
        );

        if frame_files.is_empty() {
            return Err(Error::InvalidObject("Animation contains no frames"));
        }

        let frames = frame_files.iter()
            .map(|entry| fs::read_to_string(entry.path()).map_err(Error::IO))
            .collect::<Result<_, _>>()?;

        Ok(Self { name, metadata, frames })
    }
}

#[derive(Deserialize, Debug)]
pub struct StateMetadata {
    pub animation: String,
    pub update_delay: u64,
}

impl StateMetadata {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let toml_string = fs::read_to_string(path)
            .map_err(Error::IO)?;

        toml::de::from_str(&toml_string).map_err(Error::TomlDeserializer)
    }
}

#[derive(Debug)]
pub struct StateEventHandlers<'lua> {
    pub init: Option<Function<'lua>>,
    pub update: Option<Function<'lua>>,
    pub key_down: Option<Function<'lua>>,
    pub key_up: Option<Function<'lua>>,
}

impl<'lua> StateEventHandlers<'lua> {
    pub fn get_from(lua: &'lua Lua) -> Self {
        let globals = lua.globals();

        Self {
            init: globals.get("Init").ok(),
            update: globals.get("Update").ok(),
            key_down: globals.get("Key_down").ok(),
            key_up: globals.get("Key_up").ok(),
        }
    }
}

#[derive(Debug)]
pub struct State<'lua> {
    pub metadata: StateMetadata,
    pub event_handlers: StateEventHandlers<'lua>,

    pub init_function: Function<'lua>,
    pub update_function: Function<'lua>,
}

impl<'lua> State<'lua> {
    pub fn load(lua: &'lua Lua, path: &Path) -> Result<Self, Error> {
        let metadata = StateMetadata::load(path.join("meta.toml").as_path())?;

        let name = path.file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| Error::InvalidFileName)?;

        let lua_script = fs::read_to_string(path.join("state.lua")).map_err(Error::IO)?;

        lua.load(&lua_script)
            .set_name(name)
            .exec()
            .map_err(Error::Lua)?;

        let init_function: Function = lua.globals().get("Init").map_err(Error::Lua)?;
        let update_function: Function = lua.globals().get("Update").map_err(Error::Lua)?;

        let event_handlers = StateEventHandlers::get_from(lua);

        Ok(Self{ metadata, event_handlers, init_function, update_function })
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct PetMetadata {
    pub name: String,
    pub description: String,
    pub default_state: String,
    pub global_tick_delay: u64,
}

impl PetMetadata {
    pub fn load(path: PathBuf) -> Result<Self, Error> {
        let toml_string = match fs::read_to_string(path) {
            Ok(val) => val,
            Err(e) => return Err(Error::IO(e)),
        };

        match toml::de::from_str(toml_string.as_str()) {
            Ok(val) => Ok(val),
            Err(e) => Err(Error::TomlDeserializer(e)),
        }
    }
}

pub struct Pet<'lua> {
    pub metadata: PetMetadata,
    pub animations: HashMap<String, Animation>,
    pub states: HashMap<String, State<'lua>>,
}

impl<'lua> Pet<'lua> {
    pub fn load(lua: &'lua Lua, path: &Path) -> Result<Pet<'lua>, Error> {
        let metadata = PetMetadata::load(path.join("meta.toml") )?;

        let animation_dirs: Vec<_> = fs::read_dir(path.join("anim"))
            .map_err(Error::IO)?
            .filter_map(|d|
                d.map_err(Error::IO)
                    .ok()
                    .filter(|entry|
                        entry.path().is_dir())
            )
            .collect();

        let mut animations = HashMap::new();

        for animation_path in animation_dirs {
            let name = animation_path.file_name().into_string().map_err(|s| Error::Utf8(s.into()))?;
            let animation = Animation::load(animation_path.path().as_path())?;

            animations.insert(name, animation);
        }

        let state_dirs: Vec<_> = fs::read_dir(path.join("state"))
            .map_err(Error::IO)?
            .filter_map(|d|
                d.map_err(Error::IO)
                    .ok()
                    .filter(|entry|
                        entry.path().is_dir())
            )
            .collect();

        let mut states = HashMap::new();

        for state_path in state_dirs {
            let name = state_path.file_name().into_string().map_err(|s| Error::Utf8(s.into()))?;
            let state = State::load(lua, state_path.path().as_path())?;

            states.insert(name, state);
        }

        Ok(Self {
            metadata,
            animations,
            states,
        })
    }
}

