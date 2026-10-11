/**
 * Network Diagnostics Types
 * 
 * TypeScript interfaces for network diagnostic tools.
 */

import type * as B from '@/bindings';
/** Detailed network interface information */
export type NetworkInterfaceDetails = B.NetworkInterfaceDetails;

/** Ping result for a single host */
export type PingResult = B.PingResult;

/** Traceroute result */
export type TracerouteResult = B.TracerouteResult;

/** DNS lookup result */
export type DnsLookupResult = B.DnsLookupResult;

/** WiFi information */
export type WifiInfo = B.WifiInfo;

