use alloc::alloc::Allocator;
use alloc::boxed::Box;

pub struct StackLinkedList<T, A: Allocator + Clone> {
    head: Option<Box<Node<T, A>, A>>,
    alloc: A,
}

pub struct Node<T, A: Allocator + Clone> {
    pub value: T,
    pub next: Option<Box<Node<T, A>, A>>,
}

impl<T, A: Allocator + Clone> StackLinkedList<T, A> {
    pub fn empty(alloc: A) -> Self {
        Self { head: None, alloc }
    }
    pub fn new(value: T, alloc: A) -> Self {
        Self {
            head: Some(Box::new_in(Node::new(value), alloc.clone())),
            alloc,
        }
    }

    pub fn push(&mut self, value: T) {
        let mut new = Box::new_in(Node::new(value), self.alloc.clone());
        let cur = self.head.take();
        new.next = cur;
        self.head = Some(new);
    }

    pub fn pop(&mut self) -> Option<T> {
        match self.head.take() {
            None => None,
            Some(mut head) => {
                self.head = head.next.take();

                Some(head.value)
            }
        }
    }

    pub fn peek(&self) -> &Option<Box<Node<T, A>, A>> {
        &self.head
    }

    pub fn iter(&self) -> StackLinkedListIter<T, A> {
        StackLinkedListIter { next: &self.head }
    }
}

impl<T, A: Allocator + Clone> Node<T, A> {
    fn new(value: T) -> Self {
        Self { value, next: None }
    }
}

pub struct StackLinkedListIter<'a, T, A: Allocator + Clone> {
    next: &'a Option<Box<Node<T, A>, A>>,
}

impl<'a, T, A: Allocator + Clone> IntoIterator for &'a StackLinkedList<T, A> {
    type Item = &'a T;

    type IntoIter = StackLinkedListIter<'a, T, A>;

    fn into_iter(self) -> Self::IntoIter {
        StackLinkedListIter { next: &self.head }
    }
}

impl<'a, T, A: Allocator + Clone> Iterator for StackLinkedListIter<'a, T, A> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next {
            Some(x) => {
                self.next = &x.next;
                Some(&x.value)
            }
            None => None,
        }
    }
}
