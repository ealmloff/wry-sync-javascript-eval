// SlotMap implementation for JS heap types
class JSHeap {
  private slots: (unknown | undefined)[];
  private freeIds: number[];
  private maxId: number;

  constructor() {
    this.slots = [];
    this.freeIds = [];
    this.maxId = 0;
  }

  insert(value: unknown): number {
    let id: number;
    if (this.freeIds.length > 0) {
      id = this.freeIds.pop()!;
    } else {
      id = this.maxId;
      this.maxId++;
    }
    this.slots[id] = value;
    return id;
  }

  get(id: number): unknown | undefined {
    return this.slots[id];
  }

  remove(id: number): unknown | undefined {
    const value = this.slots[id];
    if (value !== undefined) {
      this.slots[id] = undefined;
      this.freeIds.push(id);
    }
    return value;
  }

  has(id: number): boolean {
    return this.slots[id] !== undefined;
  }
}

export { JSHeap };