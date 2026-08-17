public final class CallbackDispatchBench {
    private static long profiledLeaf(long value) {
        value ^= value >>> 17;
        value *= 0xed5ad4bbL;
        value ^= value >>> 11;
        return value;
    }

    private static long execute(int iterations, long seed) {
        long value = seed;
        for (int index = 0; index < iterations; index++) {
            value = profiledLeaf(value + index);
        }
        return value;
    }

    private static Measurement executeParallel(
        int threads,
        int iterationsPerThread,
        long seed
    ) throws InterruptedException {
        java.util.concurrent.CountDownLatch ready =
            new java.util.concurrent.CountDownLatch(threads);
        java.util.concurrent.CountDownLatch start =
            new java.util.concurrent.CountDownLatch(1);
        java.util.concurrent.CountDownLatch done =
            new java.util.concurrent.CountDownLatch(threads);
        Thread[] workers = new Thread[threads];
        long[] results = new long[threads];
        boolean[] interrupted = new boolean[threads];

        for (int worker = 0; worker < threads; worker++) {
            final int index = worker;
            workers[worker] = new Thread(() -> {
                ready.countDown();
                try {
                    start.await();
                    results[index] = execute(
                        iterationsPerThread,
                        seed + 0x9e37_79b9_7f4a_7c15L * index
                    );
                } catch (InterruptedException error) {
                    interrupted[index] = true;
                    Thread.currentThread().interrupt();
                } finally {
                    done.countDown();
                }
            }, "callback-bench-" + worker);
            workers[worker].start();
        }

        ready.await();
        long started = System.nanoTime();
        start.countDown();
        done.await();
        long elapsed = System.nanoTime() - started;

        long checksum = 0;
        for (int worker = 0; worker < threads; worker++) {
            workers[worker].join();
            if (interrupted[worker]) {
                throw new IllegalStateException("benchmark worker was interrupted");
            }
            checksum ^= Long.rotateLeft(results[worker], worker);
        }
        return new Measurement(elapsed, checksum);
    }

    private static final class Measurement {
        private final long elapsed;
        private final long checksum;

        private Measurement(long elapsed, long checksum) {
            this.elapsed = elapsed;
            this.checksum = checksum;
        }
    }

    public static void main(String[] args) throws InterruptedException {
        if (args.length != 3) {
            throw new IllegalArgumentException(
                "usage: CallbackDispatchBench WARMUP ITERATIONS_PER_THREAD THREADS"
            );
        }

        int warmupIterations = Integer.parseInt(args[0]);
        int iterationsPerThread = Integer.parseInt(args[1]);
        int threads = Integer.parseInt(args[2]);
        if (warmupIterations < 1 || iterationsPerThread < 1 || threads < 1) {
            throw new IllegalArgumentException("iteration counts and threads must be positive");
        }

        long seed = execute(warmupIterations, 0x1234_5678_9abc_def0L);
        Measurement measurement;
        if (threads == 1) {
            long start = System.nanoTime();
            long checksum = execute(iterationsPerThread, seed);
            measurement = new Measurement(System.nanoTime() - start, checksum);
        } else {
            measurement = executeParallel(threads, iterationsPerThread, seed);
        }

        long iterations = (long) iterationsPerThread * threads;
        double nsPerCall = (double) measurement.elapsed / iterations;
        double callsPerSecond = (double) iterations * 1_000_000_000.0 / measurement.elapsed;

        System.out.println("benchmark=callback_dispatch");
        System.out.println("warmup_iterations=" + warmupIterations);
        System.out.println("threads=" + threads);
        System.out.println("iterations_per_thread=" + iterationsPerThread);
        System.out.println("iterations=" + iterations);
        System.out.println("elapsed_ns=" + measurement.elapsed);
        System.out.printf(java.util.Locale.ROOT, "ns_per_call=%.3f%n", nsPerCall);
        System.out.printf(java.util.Locale.ROOT, "calls_per_second=%.1f%n", callsPerSecond);
        System.out.println("checksum=" + measurement.checksum);
    }
}
