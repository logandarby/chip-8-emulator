// Common util module for macros

#[macro_export]
macro_rules! validated_struct {
    (
        $(#[$attr:meta])*
        $vis:vis struct $name:ident($inner_vis:vis $type:ty) {
            validate: $validator:expr
        }
    ) => {
        $(#[$attr])*
        #[derive(Clone, Copy, Debug)]
        $vis struct $name($type);

        impl $name {
            #[allow(dead_code)]
            pub fn new(value: $type) -> Result<Self, String> {
                let validator: fn($type) -> Result<(), String> = $validator;
                validator(value)?;
                Ok(Self(value))
            }

            pub fn get(&self) -> $type {
                self.0
            }
        }

        impl std::ops::Deref for $name {
            type Target = $type;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}
