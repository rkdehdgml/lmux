import { expect, test } from "vitest";
import { latestOnly } from "./serial";

test("보내는 동안 들어온 값은 마지막 것만 순서대로 보낸다", async () => {
  const sent: number[] = [];
  let release = () => {};
  const send = latestOnly((n: number) => {
    sent.push(n);
    return new Promise<void>((resolve) => (release = resolve));
  });
  const done = send(1);
  void send(2);
  void send(3);
  expect(sent).toEqual([1]);
  release();
  await new Promise((resolve) => setTimeout(resolve));
  expect(sent).toEqual([1, 3]);
  release();
  await done;
});

test("보내다 실패해도 기다리던 값은 보낸다", async () => {
  const sent: number[] = [];
  const send = latestOnly(async (n: number) => {
    sent.push(n);
    if (n === 1) throw new Error("fail");
  });
  const first = send(1);
  void send(2).catch(() => {});
  await first.catch(() => {});
  expect(sent).toEqual([1, 2]);
});

test("실패한 뒤에도 다음 값을 보낸다", async () => {
  const sent: number[] = [];
  const send = latestOnly(async (n: number) => {
    sent.push(n);
    if (n === 1) throw new Error("fail");
  });
  await expect(send(1)).rejects.toThrow("fail");
  await send(2);
  expect(sent).toEqual([1, 2]);
});
