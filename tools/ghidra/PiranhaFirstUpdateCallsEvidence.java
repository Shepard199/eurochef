import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.symbol.Reference;
public class PiranhaFirstUpdateCallsEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  Address s=toAddr(0x00467C60L); disassemble(s);
  InstructionIterator it=currentProgram.getListing().getInstructions(s,true);
  int n=0;
  while(it.hasNext()&&n<240){
   Instruction ins=it.next();
   if(ins.getAddress().getOffset()>=0x00468080L) break;
   if(ins.getFlowType().isCall() || ins.toString().contains("0x654") || ins.toString().contains("0x655") || ins.toString().contains("0x128"))
     println(ins.getAddress()+"  "+ins);
   for(Reference r:ins.getReferencesFrom()) if(r.getReferenceType().isCall()) println("  CALLREF -> "+r.getToAddress());
   n++;
  }
 }
}