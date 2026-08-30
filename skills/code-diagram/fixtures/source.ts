export function submit(input: string) {
  const request = normalize(input);
  const accepted = api.send(request);
  return accepted.id;
}

export function enqueue(id: string) {
  queue.push({ id, attempts: 0 });
}

export const poll = (id: string) => api.status(id);
