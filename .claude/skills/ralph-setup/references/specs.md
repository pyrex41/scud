# Writing JTBD Specifications

Jobs to Be Done (JTBD) format for Ralph specs.

## Spec File Structure

One file per topic in `specs/` directory:

```
specs/
├── authentication.md
├── color-extraction.md
├── payment-processing.md
└── user-profiles.md
```

## Topic Scope Test

A topic is properly scoped if describable in **one sentence without conjunction**:

**Good:**
- "The color extraction system analyzes images for dominant colors."
- "Authentication verifies user identity via OAuth providers."
- "The notification system delivers alerts via email and push."

**Bad (multiple topics):**
- "Authentication, profiles, and billing" → Split into 3 specs
- "Users can login and manage their settings and view reports" → Split

## Spec Template

```markdown
# [Topic Name]

## Job to Be Done
When [situation], I want to [motivation], so I can [outcome].

## Functional Requirements
- [ ] FR-1: [Specific, testable requirement]
- [ ] FR-2: [Specific, testable requirement]
- [ ] FR-3: [Specific, testable requirement]

## Non-Functional Requirements
- [ ] NFR-1: [Performance/security/reliability requirement]
- [ ] NFR-2: [Constraint or quality attribute]

## Acceptance Criteria
1. Given [precondition], when [action], then [result]
2. Given [precondition], when [action], then [result]

## Out of Scope
- [Explicitly excluded functionality]
- [Related but separate concerns]

## Dependencies
- Requires: [other spec or external system]
- Blocked by: [prerequisite work]

## Open Questions
- [Unresolved decisions - Ralph will ask or make assumptions]
```

## Example Spec

```markdown
# Color Extraction

## Job to Be Done
When uploading an image, I want to extract dominant colors, so I can generate coordinated color palettes.

## Functional Requirements
- [ ] FR-1: Accept PNG, JPEG, WebP images up to 10MB
- [ ] FR-2: Extract top 5 dominant colors using k-means clustering
- [ ] FR-3: Return colors in hex, RGB, and HSL formats
- [ ] FR-4: Generate complementary palette suggestions

## Non-Functional Requirements
- [ ] NFR-1: Process images under 2 seconds for images < 5MB
- [ ] NFR-2: Handle concurrent requests without memory exhaustion

## Acceptance Criteria
1. Given a JPEG image, when processed, then returns 5 hex colors
2. Given an invalid file type, when uploaded, then returns 400 error
3. Given a 10MB image, when processed, then completes within 5 seconds

## Out of Scope
- Image resizing or manipulation
- Color naming (just numerical values)
- Palette storage/history

## Dependencies
- Requires: Image processing library (Sharp, Pillow, or similar)
```

## Writing Tips

1. **Be specific**: "Returns error" → "Returns 400 with JSON error body"
2. **Be testable**: Every requirement should have a clear pass/fail condition
3. **Be complete**: Ralph will implement exactly what's specified
4. **Capture constraints**: Performance limits, size limits, rate limits
5. **Note assumptions**: Make implicit requirements explicit

## What Ralph Does With Specs

1. **Planning mode**: Analyzes specs, identifies gaps, generates tasks
2. **Building mode**: Implements requirements, checks off completed items
3. **Validation**: Uses acceptance criteria to verify implementation

## Spec Maintenance

- Update specs when requirements change
- Mark completed requirements with `[x]`
- Add discovered requirements during implementation
- Move resolved questions to requirements

Ralph treats specs as source of truth. Keep them current.
