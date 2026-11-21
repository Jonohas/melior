import { type ClassValue, clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

/** Joins class names and lets a later Tailwind utility win over an earlier conflicting one. */
export const cn = (...inputs: ClassValue[]): string => twMerge(clsx(inputs));

/** The first two letters of a name, upper-cased, for an avatar without a picture. */
export const initialsOf = (name: string): string => name.slice(0, 2).toUpperCase();
