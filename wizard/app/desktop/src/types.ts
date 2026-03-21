/**
 * TypeScript type definitions for Wizard review and artifact system
 * These mirror the Rust types defined in wizard-proto
 */

export interface Artifact {
  id: string;
  unit_id: string;
  artifact_type: ArtifactType;
  path: string;
  size: number;
  created_at: string | number;
  checksum?: string;
  metadata: Record<string, string>;
  reviewed: boolean;
  review_status?: ReviewDecision;
}

export type ArtifactType = 
  | { CodeFile: { language: string } }
  | 'Documentation'
  | 'Config' 
  | 'Test'
  | 'Build'
  | 'Log'
  | 'Image'
  | { Data: { format: string } }
  | { Other: { description: string } };

export interface Review {
  id: string;
  unit_id: string;
  review_type: ReviewType;
  status: ReviewStatus;
  requested_at: string | number;
  completed_at?: string | number;
  decision?: ReviewDecision;
  notes?: string;
  artifacts: string[]; // artifact IDs
  checklist: ReviewChecklistItem[];
}

export type ReviewType =
  | 'Code'
  | 'Documentation' 
  | 'Test'
  | 'Architecture'
  | 'Performance'
  | 'Security'
  | 'Completion'
  | { Artifact: { artifact_id: string } };

export type ReviewStatus =
  | 'Pending'
  | 'InProgress'
  | 'Completed'
  | 'Cancelled'
  | { Blocked: { reason: string } };

export type ReviewDecision =
  | 'Approve'
  | { RequestChanges: { required_changes: string[] } }
  | { Reject: { reason: string } }
  | { NeedsInfo: { questions: string[] } }
  | { Defer: { until?: string | number } };

export interface ReviewChecklistItem {
  description: string;
  checked: boolean;
  notes?: string;
  required: boolean;
}

export type VerificationStatus =
  | 'Passed'
  | 'Failed'
  | 'InProgress'
  | { Skipped: { reason: string } }
  | { Error: { message: string } };

export interface VerificationDetails {
  checks: VerificationCheck[];
  summary: string;
  verified_artifacts: string[];
  issues: VerificationIssue[];
}

export interface VerificationCheck {
  name: string;
  status: VerificationStatus;
  description: string;
  duration: number; // milliseconds
  output?: string;
}

export interface VerificationIssue {
  severity: IssueSeverity;
  message: string;
  file?: string;
  line?: number;
  suggestion?: string;
}

export type IssueSeverity = 
  | 'Info'
  | 'Warning' 
  | 'Error'
  | 'Critical';

// Event types for real-time updates
export interface ArtifactGeneratedEvent {
  artifact_id: string;
  unit_id: string;
  artifact_type: ArtifactType;
  path: string;
  timestamp: string | number;
}

export interface ReviewRequestedEvent {
  review_id: string;
  unit_id: string;
  review_type: ReviewType;
  timestamp: string | number;
}

export interface ReviewCompletedEvent {
  review_id: string;
  decision: ReviewDecision;
  notes?: string;
  timestamp: string | number;
}

export interface VerificationResultEvent {
  unit_id: string;
  verification_id: string;
  result: VerificationStatus;
  details: VerificationDetails;
  timestamp: string | number;
}

// Command types for interacting with backend
export type ReviewCommand =
  | { RequestReview: { unit_id: string; review_type: ReviewType } }
  | { CompleteReview: { review_id: string; decision: ReviewDecision; notes?: string } }
  | { GetArtifacts: { unit_id?: string } }
  | { GetReviewHistory: { unit_id?: string } }
  | { VerifyUnit: { unit_id: string } };