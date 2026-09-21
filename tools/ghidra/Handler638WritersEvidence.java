import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import java.util.*;
public class Handler638WritersEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  Set<Function> fs=new LinkedHashSet<>();
  InstructionIterator it=currentProgram.getListing().getInstructions(true);
  while(it.hasNext()){
   Instruction ins=it.next(); long a=ins.getAddress().getOffset();
   if(a<0x00450000L||a>=0x00470000L)continue;
   String s=ins.toString().toLowerCase();
   if(!(s.contains("0x638")||s.contains("+ 0x638")||s.contains("+0x638")))continue;
   Function f=getFunctionContaining(ins.getAddress());
   println(ins.getAddress()+"  "+ins+"  "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
   if(f!=null)fs.add(f);
  }
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  for(Function f:fs){println("\n=== "+f.getName()+" "+f.getEntryPoint()+" ===");DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
  d.dispose();
 }
}