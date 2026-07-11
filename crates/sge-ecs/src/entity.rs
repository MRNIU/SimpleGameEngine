// Copyright The SimpleGameEngine Contributors

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entity {
    index: u32,
    generation: u64,
}

#[derive(Debug, Default)]
pub(crate) struct EntityAllocator {
    slots: Vec<EntitySlot>,
    free: Vec<u32>,
}

#[derive(Debug)]
struct EntitySlot {
    generation: u64,
    alive: bool,
}

impl EntityAllocator {
    pub(crate) fn spawn(&mut self) -> Entity {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.alive = true;
            return Entity {
                index,
                generation: slot.generation,
            };
        }

        let index = u32::try_from(self.slots.len()).expect("entity slot index exhausted");
        self.slots.push(EntitySlot {
            generation: 0,
            alive: true,
        });
        Entity {
            index,
            generation: 0,
        }
    }

    pub(crate) fn is_alive(&self, entity: Entity) -> bool {
        self.slots
            .get(entity.index as usize)
            .is_some_and(|slot| slot.alive && slot.generation == entity.generation)
    }

    pub(crate) fn despawn(&mut self, entity: Entity) -> bool {
        let Some(slot) = self.slots.get_mut(entity.index as usize) else {
            return false;
        };
        if !slot.alive || slot.generation != entity.generation {
            return false;
        }

        slot.alive = false;
        if let Some(generation) = slot.generation.checked_add(1) {
            slot.generation = generation;
            self.free.push(entity.index);
        }
        true
    }

    pub(crate) fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.alive)
            .map(|(index, slot)| Entity {
                index: u32::try_from(index).expect("entity slot index exhausted"),
                generation: slot.generation,
            })
    }
}
