use std::marker::PhantomData;

use crate::tree_f32::{FloatReadAccessor, FloatValidatedTree, TreeData};
use crate::tree_vec3f::{Vec3fReadAccessor, Vec3fValidatedTree};
use crate::types::Vec3f;

enum Accessor<'a> {
    Float(FloatReadAccessor<'a>),
    Vec3f(Vec3fReadAccessor<'a>),
}

enum Validated<'a> {
    Float(FloatValidatedTree<'a>),
    Vec3f(Vec3fValidatedTree<'a>),
}

/// A validated, typed view of a NanoVDB tree.
pub struct ValidatedTree<'a, V = f32> {
    inner: Validated<'a>,
    marker: PhantomData<V>,
}

impl<'a> ValidatedTree<'a, f32> {
    pub fn new(bytes: &'a [u8]) -> Option<Self> {
        Some(Self {
            inner: Validated::Float(FloatValidatedTree::new(bytes)?),
            marker: PhantomData,
        })
    }

    pub fn sample(&self, xyz: [f32; 3]) -> Option<f32> {
        match &self.inner {
            Validated::Float(tree) => tree.sample(xyz),
            Validated::Vec3f(_) => unreachable!(),
        }
    }
}

impl<'a> ValidatedTree<'a, Vec3f> {
    pub fn new(bytes: &'a [u8]) -> Option<Self> {
        Some(Self {
            inner: Validated::Vec3f(Vec3fValidatedTree::new(bytes)?),
            marker: PhantomData,
        })
    }

    pub fn get_value(&self, ijk: [i32; 3]) -> Vec3f {
        match &self.inner {
            Validated::Vec3f(tree) => tree.get_value(ijk),
            Validated::Float(_) => unreachable!(),
        }
    }
}

/// Typed random-access reader for NanoVDB trees.
pub struct ReadAccessor<'a, V = f32> {
    inner: Accessor<'a>,
    marker: PhantomData<V>,
}

impl<'a> ReadAccessor<'a, f32> {
    pub fn from_grid_bytes(bytes: &'a [u8]) -> Option<Self> {
        Some(Self {
            inner: Accessor::Float(FloatReadAccessor::from_grid_bytes(bytes)?),
            marker: PhantomData,
        })
    }

    pub fn parse_tree_data(bytes: &[u8]) -> Option<(TreeData, f32)> {
        FloatReadAccessor::parse_tree_data(bytes)
    }

    pub fn with_tree_data(bytes: &'a [u8], tree: TreeData, background: f32) -> Self {
        Self {
            inner: Accessor::Float(FloatReadAccessor::with_tree_data(bytes, tree, background)),
            marker: PhantomData,
        }
    }

    pub fn background(&self) -> f32 {
        match &self.inner {
            Accessor::Float(accessor) => accessor.background(),
            Accessor::Vec3f(_) => unreachable!(),
        }
    }

    pub fn get_value(&mut self, ijk: [i32; 3]) -> f32 {
        match &mut self.inner {
            Accessor::Float(accessor) => accessor.get_value(ijk),
            Accessor::Vec3f(_) => unreachable!(),
        }
    }

    pub fn is_active(&mut self, ijk: [i32; 3]) -> bool {
        match &mut self.inner {
            Accessor::Float(accessor) => accessor.is_active(ijk),
            Accessor::Vec3f(_) => unreachable!(),
        }
    }
}

impl<'a> ReadAccessor<'a, Vec3f> {
    pub fn from_grid_bytes(bytes: &'a [u8]) -> Option<Self> {
        Some(Self {
            inner: Accessor::Vec3f(Vec3fReadAccessor::from_grid_bytes(bytes)?),
            marker: PhantomData,
        })
    }

    pub fn background(&self) -> Vec3f {
        match &self.inner {
            Accessor::Vec3f(accessor) => accessor.background(),
            Accessor::Float(_) => unreachable!(),
        }
    }

    pub fn get_value(&self, ijk: [i32; 3]) -> Vec3f {
        match &self.inner {
            Accessor::Vec3f(accessor) => accessor.get_value(ijk),
            Accessor::Float(_) => unreachable!(),
        }
    }
}
