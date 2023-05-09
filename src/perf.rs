use std::collections::VecDeque;
use systemstat::ByteSize;

pub struct PerfData {
    pub(crate) cpu_usage: u16,
    pub(crate) mem_usage: (ByteSize, ByteSize),
}

pub struct PerfLog<T> {
    data: VecDeque<T>,
    capacity: usize,
}

impl<T> PerfLog<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, value: T) {
        if self.data.len() == self.capacity {
            self.data.pop_front();
        }
        self.data.push_back(value);
    }

    pub fn last(&self) -> Option<&T> {
        self.data.back()
    }

    pub fn iter(&self) -> PerfLogIter<T> {
        PerfLogIter {
            data: &self.data,
            index: 0,
        }
    }
}

pub struct PerfLogIter<'a, T> {
    data: &'a VecDeque<T>,
    index: usize,
}

impl<'a, T> Iterator for PerfLogIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.data.len() {
            let result = &self.data[self.index];
            self.index += 1;
            Some(result)
        } else {
            None
        }
    }
}
