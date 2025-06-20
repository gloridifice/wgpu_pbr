use std::any::TypeId;
use std::cell::{RefCell, RefMut};
use std::collections::{HashMap, HashSet, VecDeque};
use std::mem::swap;
use std::rc::{Rc, Weak};

use bevy_ecs::resource::Resource;
use bevy_ecs::world::{FromWorld, World};

#[derive(Debug)]
pub struct Node<T> {
    value: T,
    parents: Vec<Weak<RefCell<Node<T>>>>, // 父节点（弱引用）
    children: Vec<Rc<RefCell<Node<T>>>>,  // 子节点（强引用）
}

impl<T> Node<T> {
    fn new(value: T) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Node {
            value,
            parents: Vec::new(),
            children: Vec::new(),
        }))
    }

    // 添加子节点，并建立双向关系
    fn add_child(parent: &Rc<RefCell<Node<T>>>, child: &Rc<RefCell<Node<T>>>) {
        parent.borrow_mut().children.push(child.clone());
        child.borrow_mut().parents.push(Rc::downgrade(parent));
    }
}

pub struct RcBfsIterator<'a, T> {
    queue: VecDeque<Rc<RefCell<Node<T>>>>,
    visited: HashSet<*const Node<T>>,
    _marker: std::marker::PhantomData<&'a mut T>,
}

impl<'a, T> RcBfsIterator<'a, T> {
    pub fn new(start: &Rc<RefCell<Node<T>>>) -> Self {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        visited.insert(Rc::as_ptr(&start) as *const Node<T>);
        queue.push_back(start.clone());
        RcBfsIterator {
            queue,
            visited,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<'a, T> Iterator for RcBfsIterator<'a, T> {
    type Item = Rc<RefCell<Node<T>>>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(node) = self.queue.pop_front() {
            // 处理子节点的入队逻辑
            for child in &node.borrow().children {
                let child_ptr = Rc::as_ptr(child) as *const Node<T>;
                if !self.visited.contains(&child_ptr) {
                    self.visited.insert(child_ptr);
                    self.queue.push_back(child.clone());
                }
            }

            Some(node.clone())
        } else {
            None
        }
    }
}

type Value = (TypeId, Option<Box<dyn FnOnce(&mut World)>>);
type ResNode = Node<Value>;
type ResNodeRef = Rc<RefCell<ResNode>>;

pub struct ResourceGraph {
    hashmap: HashMap<TypeId, ResNodeRef>,
    root: ResNodeRef,
}

pub struct Adder<'a> {
    graph: &'a mut ResourceGraph,
    node: ResNodeRef,
}

impl ResourceGraph {
    pub fn new() -> Self {
        Self {
            hashmap: HashMap::new(),
            root: Node::new((TypeId::of::<bool>(), None)),
        }
    }

    pub fn insert<T: Resource + FromWorld>(&mut self) -> Adder {
        let node = self.get_or_create_node::<T>();
        node.borrow_mut().value.1 = Some(Box::new(|world| {
            world.init_resource::<T>();
        }));
        Adder {
            graph: self,
            node: node,
        }
    }

    fn get_or_create_node<T: Resource + FromWorld>(&mut self) -> ResNodeRef {
        let type_id = TypeId::of::<T>();
        if !self.hashmap.contains_key(&type_id) {
            let node = ResNode::new((type_id, None));
            Node::add_child(&self.root, &node);
            self.hashmap.insert(type_id, node);
        }
        Rc::clone(&self.hashmap[&type_id])
    }
}

impl IntoIterator for ResourceGraph {
    type Item = Box<dyn FnOnce(&mut World) + 'static>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let mut ret = vec![];
        for item in RcBfsIterator::new(&self.root) {
            let a = &mut item.borrow_mut().value.1;
            let mut closure = None;
            swap(&mut closure, a);
            if let Some(closure) = closure {
                ret.push(closure);
            }
        }
        ret.into_iter()
    }
}

impl<'a> Adder<'a> {
    pub fn before<T: Resource + FromWorld>(&mut self) -> &mut Self {
        let child = self.graph.get_or_create_node::<T>();
        Node::add_child(&self.node, &child);
        self
    }

    pub fn after<T: Resource + FromWorld>(&mut self) -> &mut Self {
        let parent = self.graph.get_or_create_node::<T>();
        Node::add_child(&parent, &self.node);
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_graph() {
        // 构建一个多父节点的 DAG：
        //       A
        //     /   \
        //    B     C
        //     \   /
        //       D
        let a = Node::new("A");
        let b = Node::new("B");
        let c = Node::new("C");
        let d = Node::new("D");

        Node::add_child(&a, &b); // A -> B
        Node::add_child(&a, &c); // A -> C
        Node::add_child(&b, &d); // B -> D
        Node::add_child(&c, &d); // C -> D （D 有两个父节点）

        // 验证 D 的父节点数量
        assert_eq!(d.borrow().parents.len(), 2); // D 有 B 和 C 两个父节点
    }
}
