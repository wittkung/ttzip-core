// SPDX-License-Identifier: Apache-2.0

package org.junit.jupiter.api;

import java.util.Arrays;
import java.util.Objects;

public final class Assertions {

    private Assertions() {}

    @FunctionalInterface
    public interface Executable {
        void execute() throws Throwable;
    }

    public static void assertTrue(boolean condition) {
        assertTrue(condition, "Expected true but was false");
    }

    public static void assertTrue(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void assertFalse(boolean condition) {
        assertFalse(condition, "Expected false but was true");
    }

    public static void assertFalse(boolean condition, String message) {
        if (condition) throw new AssertionError(message);
    }

    public static void assertEquals(Object expected, Object actual) {
        assertEquals(expected, actual, "Expected: " + expected + " but was: " + actual);
    }

    public static void assertEquals(Object expected, Object actual, String message) {
        if (!Objects.equals(expected, actual)) {
            throw new AssertionError(message + " (expected: <" + expected + ">, actual: <" + actual + ">)");
        }
    }

    public static void assertNotEquals(Object unexpected, Object actual) {
        assertNotEquals(unexpected, actual, "Expected values to be different but both were: " + actual);
    }

    public static void assertNotEquals(Object unexpected, Object actual, String message) {
        if (Objects.equals(unexpected, actual)) {
            throw new AssertionError(message + " (unexpected: <" + unexpected + ">, actual: <" + actual + ">)");
        }
    }

    public static void assertNotNull(Object actual) {
        assertNotNull(actual, "Expected non-null object");
    }

    public static void assertNotNull(Object actual, String message) {
        if (actual == null) throw new AssertionError(message);
    }

    public static void assertNull(Object actual) {
        assertNull(actual, "Expected null object");
    }

    public static void assertNull(Object actual, String message) {
        if (actual != null) throw new AssertionError(message);
    }

    public static void assertArrayEquals(byte[] expected, byte[] actual) {
        assertArrayEquals(expected, actual, "Byte arrays differ");
    }

    public static void assertArrayEquals(byte[] expected, byte[] actual, String message) {
        if (!Arrays.equals(expected, actual)) {
            throw new AssertionError(message + " (arrays differ in content or length)");
        }
    }

    public static void assertDoesNotThrow(Executable executable) {
        assertDoesNotThrow(executable, "Expected execution not to throw");
    }

    public static void assertDoesNotThrow(Executable executable, String message) {
        try {
            executable.execute();
        } catch (Throwable t) {
            throw new AssertionError(message + " (unexpected exception: " + t.getMessage() + ")", t);
        }
    }
}
