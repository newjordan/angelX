"""Small, editable local-angle clips for the quick rigid-pixel rig.

Degrees are clockwise in image space; zero is the fitted source pose. Timings
are Delve game ticks (30 ticks/second), not display refresh frames. Attacks
deliberately hold anticipation and recovery longer than the contact/smear,
rather than playing a constant-speed pendulum.
"""
import math

CLIPS = ('idle', 'walk', 'run', 'roll', 'hurt', 'die', 'cast', 'slash',
         'overhead', 'thrust', 'flail', 'bow', 'gun')
LOOPS = ('idle', 'walk', 'run')
TICKS_PER_SECOND = 30


def defaults(height):
    h = float(height)

    def pose(ticks=6, angles=None, offset=(0, 0), rotation=0, smear=False):
        return dict(ticks=ticks, angles=dict(angles) if angles is not None else {}, offset=list(offset),
                    rotation=rotation, smear=smear)

    def arms(f, fore=0, b=0, back_fore=0, **body):
        return dict(upper_arm_f=f, lower_arm_f=fore, upper_arm_b=b,
                    lower_arm_b=back_fore, **body)

    # A few degrees disappear under nearest sampling at 16px. Rock the upper
    # mass enough to move helmet/shoulders one pixel, leaving the feet planted.
    clips = {'idle': [pose(angles=arms(4*s, -3*abs(s), -4*s, torso=4*s,
                                             head=-s, back=3*s))
                      for s in (0, .7, 1, 0, -.7, -1)]}
    for name, swing, ticks in (('walk', 25, 6), ('run', 48, 3)):
        frames = []
        for i in range(8):
            t = 2 * math.pi * i / 8
            s, c = math.sin(t), math.cos(t)
            a = arms(-s*swing*.8, -12 if name == 'walk' else -55,
                     s*swing*.8, -12 if name == 'walk' else -55,
                     upper_leg_f=s*swing, upper_leg_b=-s*swing,
                     lower_leg_f=-max(0, -s)*swing*1.5,
                     lower_leg_b=-max(0, s)*swing*1.5,
                     foot_f=max(0, -s)*20, foot_b=max(0, s)*20,
                     torso=0 if name == 'walk' else 12, head=-3,
                     back=-5+8*c)
            frames.append(pose(ticks, a, (0, -abs(c)*h*(.025 if name == 'walk' else .06))))
        clips[name] = frames
    clips['roll'] = [pose(9, arms(20, -45, -20, -45, upper_leg_f=35,
                                          lower_leg_f=-65, torso=20), (0, h*.06))]
    for degrees in (0, 45, 90, 135, 180, 225, 270, 315, 360):
        s = math.sin(math.radians(degrees / 2)) ** 2
        clips['roll'].append(pose(3, arms(-130*s, -50*s, -120*s, -50*s,
                                                upper_leg_f=65*s, lower_leg_f=-100*s,
                                                upper_leg_b=50*s, lower_leg_b=-90*s,
                                                head=20*s, back=25*s),
                                  (0, -h*1.05*s), degrees))
    clips['roll'] += [pose(6)]
    # One hard recoil, a held stagger, then settle; translate the body, not FX.
    kick = max(1, round(h*.10))
    clips['hurt'] = [pose(3, arms(55, -65, 45, -65, torso=-24, head=-10),
                                (-max(2, round(h*.13)), 0)),
                     pose(9, arms(35, -50, 30, -50, torso=-14, head=8), (-kick/2, 0)),
                     pose(6, {'torso': -6, 'head': 3}), pose(9)]
    clips['die'] = [pose(6, {'torso': -15, 'head': -10}),
                    pose(9, arms(35, -45, 30, -35, torso=-20)),
                    *[pose(6, arms(15, -20, 25, -30, head=10, back=-15),
                           rotation=a) for a in (15, 35, 60, 82, 90)],
                    pose(30, arms(15, -20, 25, -30, head=10, back=-15), rotation=90)]
    # Opposed arm rotations lift BOTH hands above the helmet. Same-direction
    # rotations put the far hand across the face at native resolution.
    clips['cast'] = [pose(9, arms(-30, -110, 30, 110, torso=-5, head=5)),
                     pose(6, arms(-105, -50, 105, 50, head=-5)),
                     pose(9, arms(-155, -5, 155, 5, torso=-4, back=12)),
                     pose(3, arms(-140, -10, 140, 10, torso=8, head=-5)),
                     pose(9, arms(-120, -20, 120, 20)),
                     pose(6, arms(-40, -45, 40, 45)), pose(9)]
    clips['slash'] = [pose(9, arms(55, -45, -20, -40, torso=-12, head=5)),
                      pose(6, arms(75, -50, -25, -45, torso=-18, back=10)),
                      pose(3, arms(-65, -15, 20, -50, torso=10), (h*.04, 0), smear=True),
                      pose(3, arms(-140, -5, 30, -45, torso=18), (h*.06, 0), smear=True),
                      pose(9, arms(-130, -15, 15, -35, torso=12)),
                      pose(6, arms(-55, -25, 5, -20, torso=5)), pose(9)]
    clips['overhead'] = [pose(6, arms(-70, -75, -40, -85, torso=-5)),
                         pose(12, arms(-170, -15, -150, -25, torso=-12, head=-8)),
                         pose(3, arms(-95, -5, -85, -15, torso=12), smear=True),
                         pose(3, arms(-20, -10, -30, -15, torso=22), (h*.04, 0), smear=True),
                         pose(9, arms(-10, -20, -20, -25, torso=18)),
                         pose(9, arms(-5, -10, -5, -10, torso=7)), pose(9)]
    clips['thrust'] = [pose(9, arms(30, -110, -30, -70, torso=-10)),
                       pose(6, arms(40, -120, -30, -80, torso=-15), (-h*.04, 0)),
                       pose(3, arms(-90, 0, -40, -45, torso=12), (h*.08, 0), smear=True),
                       pose(6, arms(-90, 0, -40, -45, torso=15), (h*.08, 0)),
                       pose(9, arms(-50, -55, -20, -45, torso=5)), pose(9)]
    clips['flail'] = [pose(9, arms(40, -50, -20, -30, torso=-10))]
    clips['flail'] += [pose(3, arms(a, -20, -a*.2, -40, torso=8*math.sin(math.radians(a))),
                                   smear=True) for a in (0, -70, -140, -210, -280, -350)]
    clips['flail'] += [pose(9, arms(-20, -35, 10, -30, torso=10)), pose(9)]
    # Bow: reach together, draw the far hand back to the cheek, release.
    # The aiming hand stays out; torso counter-rotation sells the draw weight.
    clips['bow'] = [pose(6, arms(-45, -40, -65, -25, torso=3)),
                    pose(9, arms(-90, 0, -85, -5, torso=3, head=-3)),
                    pose(12, arms(-90, 0, -20, -155, torso=-12, head=5)),
                    pose(3, arms(-90, 0, 30, -155, torso=6, head=-3)),
                    pose(9, arms(-85, -5, 20, -100, torso=3)),
                    pose(9, arms(-35, -30, 5, -35)), pose(9)]
    # Gun: short whole-body kick (1-2px on a 16px source) and muzzle lift,
    # followed by a smaller rebound. No glow, particles or detached ghost mass.
    clips['gun'] = [pose(6, arms(-40, -40, -35, -50)),
                    pose(12, arms(-90, 0, -65, -30, head=-3)),
                    pose(3, arms(-112, -12, -85, -35, torso=-14, head=6), (-kick, 0)),
                    pose(6, arms(-98, -5, -70, -30, torso=-6, head=3), (-kick/2, 0)),
                    pose(9, arms(-90, 0, -65, -30)),
                    pose(9, arms(-35, -25, -25, -30)), pose(9)]
    return {name: clips[name] for name in CLIPS}
