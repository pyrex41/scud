package core

import "fmt"

// Validate checks all cross-object invariants in a snapshot, including DAG
// references and cycles. It is safe to call on a zero State.
func (s State) Validate() error {
	for id, g := range s.Goals {
		if id == "" || g.ID != id {
			return fmt.Errorf("goal key/id mismatch: %q", id)
		}
		if err := ValidateID(g.ID); err != nil {
			return fmt.Errorf("goal %q: %w", id, err)
		}
		if g.Status != "" && !g.Status.valid() {
			return fmt.Errorf("goal %q: invalid status %q", id, g.Status)
		}
	}
	for id, o := range s.Obligations {
		if id == "" || o.ID != id {
			return fmt.Errorf("obligation key/id mismatch: %q", id)
		}
		if err := ValidateID(o.ID); err != nil {
			return fmt.Errorf("obligation %q: %w", id, err)
		}
		if _, ok := s.Goals[o.GoalID]; !ok {
			return fmt.Errorf("obligation %q: unknown goal %q", id, o.GoalID)
		}
		if o.Status != "" && !o.Status.valid() {
			return fmt.Errorf("obligation %q: invalid status %q", id, o.Status)
		}
		seen := map[ID]bool{}
		for _, dep := range o.DependsOn {
			if dep == o.ID {
				return fmt.Errorf("obligation %q depends on itself", id)
			}
			if seen[dep] {
				return fmt.Errorf("obligation %q repeats dependency %q", id, dep)
			}
			seen[dep] = true
			d, ok := s.Obligations[dep]
			if !ok {
				return fmt.Errorf("obligation %q: unknown dependency %q", id, dep)
			}
			if d.GoalID != o.GoalID {
				return fmt.Errorf("obligation %q: dependency %q belongs to another goal", id, dep)
			}
		}
	}
	if err := validateAcyclic(s.Obligations); err != nil {
		return err
	}
	for id, ob := range s.Observations {
		if id == "" || ob.ID != id {
			return fmt.Errorf("observation key/id mismatch: %q", id)
		}
		if _, ok := s.Goals[ob.GoalID]; !ok {
			return fmt.Errorf("observation %q: unknown goal %q", id, ob.GoalID)
		}
		target, ok := s.Obligations[ob.ObligationID]
		if !ok || target.GoalID != ob.GoalID {
			return fmt.Errorf("observation %q: invalid obligation %q", id, ob.ObligationID)
		}
		if !ob.Outcome.valid() {
			return fmt.Errorf("observation %q: invalid outcome %q", id, ob.Outcome)
		}
	}
	if s.Budget.MaxSteps != 0 && s.Budget.UsedSteps > s.Budget.MaxSteps {
		return fmt.Errorf("budget steps exceed maximum")
	}
	if s.Budget.MaxCost != 0 && s.Budget.UsedCost > s.Budget.MaxCost {
		return fmt.Errorf("budget cost exceeds maximum")
	}
	return nil
}

func validateAcyclic(nodes map[ID]Obligation) error {
	marks := make(map[ID]uint8, len(nodes))
	var visit func(ID) error
	visit = func(id ID) error {
		if marks[id] == 1 {
			return fmt.Errorf("obligation dependency cycle includes %q", id)
		}
		if marks[id] == 2 {
			return nil
		}
		marks[id] = 1
		for _, dep := range nodes[id].DependsOn {
			if err := visit(dep); err != nil {
				return err
			}
		}
		marks[id] = 2
		return nil
	}
	for id := range nodes {
		if err := visit(id); err != nil {
			return err
		}
	}
	return nil
}
